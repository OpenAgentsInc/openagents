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

import {
  decodeUsageLedgerSnapshot,
  type UsageLedgerRow,
  type UsageLedgerSnapshot,
} from "../usage-ledger-contract.ts"
import {
  admitFleetRunCommand,
  admitFleetAttentionCommand,
  type FleetAttentionAction,
  type FleetAttentionCommand,
  type FleetCockpitCard,
  type FleetRunAction,
  type FleetRunCommand,
} from "../fleet-cockpit.ts"
import {
  decodeFleetRunClientProjection,
  type FleetRunClientProjection,
} from "@openagentsinc/khala-sync"

export type FleetAccountReadiness = "ready" | "credentials-missing" | "unknown"

export type FleetAccount = Readonly<{
  ref: string
  provider: string
  email: string | null
  readiness: FleetAccountReadiness
}>

/**
 * One provider rate-limit window (EP250 sidebar accounts box), mirrored from
 * the main-process contract (../provider-accounts-contract.ts). `label` is
 * pylon's own window label ("5h", "weekly", "hourly", …); `remainingPercent`
 * is what the sidebar usage bar renders.
 */
export type FleetUsageWindow = Readonly<{
  label: string
  usedPercent: number
  remainingPercent: number
  windowMinutes: number | null
  resetsAt: string | null
}>

export type FleetUsageEntry =
  | Readonly<{ state: "checking" }>
  | Readonly<{
      state: "checked"
      refreshedAt: string
      inputTokens: number | null
      outputTokens: number | null
      totalTokens: number | null
      /**
       * Rate-limit windows, present only when the provider actually reported
       * them on this probe (codex 5h/weekly). Absent means "no usage-window
       * data" — the sidebar bar then renders grayed, never a fake fill.
       */
      windows?: ReadonlyArray<FleetUsageWindow>
    }>
  | Readonly<{ state: "failed"; reason: string }>

export type FleetWorkspaceState = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable"
  /** ISO timestamp of the last successfully decoded accounts projection. */
  generatedAt: string | null
  accounts: ReadonlyArray<FleetAccount>
  usage: Readonly<Record<string, FleetUsageEntry>>
  reason: string | null
  /**
   * Session usage ledger (#8712 Lane C): main's exact per-account token
   * ledger for this desktop session, evidence-labeled "session ledger" —
   * never merged with the per-account "probe" numbers above. Its
   * `reconnectRequired` rows also drive the readiness-honesty override
   * below (probe/child evidence supersedes presence-based "ready").
   */
  ledger: UsageLedgerSnapshot | null
  cockpitCards: ReadonlyArray<FleetCockpitCard>
  cockpitAuthority: "live" | "offline" | "stale" | "revoked" | "unknown"
  authorityRuns: FleetRunClientProjection | null
}>

export const emptyFleetWorkspaceState = (): FleetWorkspaceState => ({
  phase: "idle",
  generatedAt: null,
  accounts: [],
  usage: {},
  reason: null,
  ledger: null,
  cockpitCards: [],
  cockpitAuthority: "unknown",
  authorityRuns: null,
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
    windows: Schema.Array(Schema.Struct({
      label: Schema.String,
      usedPercent: Schema.Number,
      remainingPercent: Schema.Number,
      windowMinutes: Schema.NullOr(Schema.Number),
      resetsAt: Schema.NullOr(Schema.String),
    })).pipe(Schema.optionalKey),
  }),
  Schema.Struct({ ok: Schema.Literal(false), ref: Schema.String, reason: Schema.String }),
])

const clampWindowPercent = (value: number): number => Math.min(100, Math.max(0, value))

/** Renderer-side window bounding (defense-in-depth mirror of the main side). */
const boundFleetUsageWindows = (
  windows: ReadonlyArray<FleetUsageWindow> | undefined,
): ReadonlyArray<FleetUsageWindow> =>
  (windows ?? [])
    .filter((window) => Number.isFinite(window.usedPercent) && Number.isFinite(window.remainingPercent))
    .slice(0, 4)
    .map((window) => ({
      label: window.label.slice(0, 20),
      usedPercent: clampWindowPercent(window.usedPercent),
      remainingPercent: clampWindowPercent(window.remainingPercent),
      windowMinutes:
        window.windowMinutes !== null && Number.isFinite(window.windowMinutes) && window.windowMinutes >= 0
          ? Math.floor(window.windowMinutes)
          : null,
      resetsAt: window.resetsAt === null ? null : window.resetsAt.slice(0, 40),
    }))

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
  const windows = boundFleetUsageWindows(decoded.value.windows)
  return {
    state: "checked",
    refreshedAt: decoded.value.refreshedAt.slice(0, 40),
    inputTokens: decoded.value.summary.inputTokens,
    outputTokens: decoded.value.summary.outputTokens,
    totalTokens: decoded.value.summary.totalTokens,
    ...(windows.length > 0 ? { windows } : {}),
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
        ...fleet,
        phase: "ready",
        generatedAt: projection.generatedAt,
        accounts: projection.accounts,
        // Usage evidence belongs to the projection it was checked against.
        usage: {},
        reason: null,
        // The session ledger is main-owned session truth, not projection
        // evidence — it survives an accounts refresh.
        ledger: fleet.ledger,
      }
    : { ...fleet, phase: "unavailable", reason: projection.reason }

export const withFleetLedger = (
  fleet: FleetWorkspaceState,
  value: unknown,
): FleetWorkspaceState => {
  const ledger = decodeUsageLedgerSnapshot(value)
  // A failed decode never erases previously decoded ledger evidence.
  return ledger === null ? fleet : { ...fleet, ledger }
}

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
/** Ledger push arrived (#8712 Lane C): re-pull the snapshot from the bridge. */
export const FleetLedgerUpdated = defineIntent("FleetLedgerUpdated", Schema.Null)
export const FleetRunControlRequested = defineIntent("FleetRunControlRequested", Schema.Struct({
  action: Schema.Literals(["pause", "cancel", "resume", "retry", "close"]),
  runRef: Schema.String,
}))
export const FleetAttentionDecisionRequested = defineIntent("FleetAttentionDecisionRequested", Schema.Struct({
  action: Schema.Literals(["approve", "deny"]),
  interactionRef: Schema.String,
  runRef: Schema.String,
}))

export const fleetWorkspaceIntents = [
  FleetRefreshRequested,
  FleetUsageCheckRequested,
  FleetManageAccountsRequested,
  FleetLedgerUpdated,
  FleetRunControlRequested,
  FleetAttentionDecisionRequested,
] as const

// ---------------------------------------------------------------------------
// Handlers (generic in the shell state shape, same pattern as ./settings.ts;
// shell.ts stays the single owner of its state type).
// ---------------------------------------------------------------------------

export type FleetAccountsBridge = Readonly<{
  list: () => Promise<unknown>
  usage: (ref: string) => Promise<unknown>
  fleetRuns?: () => Promise<unknown>
  /** Session usage ledger snapshot (#8712 Lane C); optional for older hosts. */
  ledger?: () => Promise<unknown>
  cockpit?: () => Promise<Readonly<{ authority: FleetWorkspaceState["cockpitAuthority"]; cards: ReadonlyArray<FleetCockpitCard> }>>
  control?: (command: FleetControlDispatch) => Promise<unknown>
  decideAttention?: (command: FleetAttentionCommand) => Promise<unknown>
}>

/**
 * The shared admitted run command plus the exact confirmed provider lane
 * (CUT-16): the durable lane fence rejects control intents whose target lane
 * mismatches the stored turn lane, so cockpit controls carry the card's
 * confirmed lane instead of letting the host default to Codex. An unknown
 * provider omits the lane.
 */
export type FleetControlDispatch = FleetRunCommand & Readonly<{
  lane?: "codex_app_server" | "claude_pylon" | "hosted_khala"
}>

export const fleetControlLaneForProvider = (
  provider: FleetCockpitCard["provider"],
): NonNullable<FleetControlDispatch["lane"]> | null =>
  provider === "codex"
    ? "codex_app_server"
    : provider === "claude"
      ? "claude_pylon"
      : provider === "openagents"
        ? "hosted_khala"
        : null

export const unavailableFleetAccountsBridge: FleetAccountsBridge = {
  list: async () => ({ ok: false, reason: "pylon_runtime_unavailable" }),
  usage: async (ref) => ({ ok: false, ref, reason: "pylon_runtime_unavailable" }),
}

export type FleetCapableState = Readonly<{ fleet: FleetWorkspaceState }>

const pullFleetLedger = <S extends FleetCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: FleetAccountsBridge,
) =>
  Effect.gen(function* () {
    if (bridge.ledger === undefined) return
    const snapshot = yield* Effect.promise(() => bridge.ledger!().catch(() => null))
    yield* SubscriptionRef.update(state, (next) => ({
      ...next,
      fleet: withFleetLedger(next.fleet, snapshot),
    }))
  })

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
    // Authority receipts are independent of the optional local Pylon account
    // projection. Read them first so a slow or unavailable local runtime can
    // never hide an authenticated server-side fleet result.
    if (bridge.fleetRuns !== undefined) {
      const result = yield* Effect.promise(() => bridge.fleetRuns!().catch(() => null))
      let projection: FleetRunClientProjection | null = null
      if (typeof result === "object" && result !== null && "state" in result && result.state === "available" && "projection" in result) {
        try { projection = decodeFleetRunClientProjection(result.projection) } catch { projection = null }
      }
      yield* SubscriptionRef.update(state, next => ({ ...next, fleet: { ...next.fleet, authorityRuns: projection } }))
    }
    const projection = decodeFleetAccountsProjection(
      yield* Effect.promise(() => bridge.list().catch(() => null)),
    )
    yield* SubscriptionRef.update(state, (next) => ({
      ...next,
      fleet: withFleetProjection(next.fleet, projection),
    }))
    yield* pullFleetLedger(state, bridge)
    if (bridge.cockpit !== undefined) {
      const cockpit = yield* Effect.promise(() => bridge.cockpit!().catch(() => ({ authority: "unknown" as const, cards: [] })))
      yield* SubscriptionRef.update(state, next => ({ ...next, fleet: { ...next.fleet, cockpitAuthority: cockpit.authority, cockpitCards: cockpit.cards.slice(0, 50) } }))
    }
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
  FleetLedgerUpdated: () => pullFleetLedger(state, bridge),
  FleetRunControlRequested: (payload: Readonly<{ action: FleetRunAction; runRef: string }>) =>
    Effect.gen(function* () {
      if (bridge.control === undefined) return
      const current = yield* SubscriptionRef.get(state)
      const card = current.fleet.cockpitCards.find(item => item.runRef === payload.runRef)
      if (card === undefined) return
      const command = admitFleetRunCommand(card, payload.action)
      if (command === null) return
      const lane = fleetControlLaneForProvider(card.provider)
      yield* Effect.promise(() => bridge.control!({
        ...command,
        ...(lane === null ? {} : { lane }),
      }).catch(() => null))
      if (bridge.cockpit === undefined) return
      const cockpit = yield* Effect.promise(() => bridge.cockpit!().catch(() => ({ authority: "unknown" as const, cards: [] })))
      yield* SubscriptionRef.update(state, next => ({ ...next, fleet: { ...next.fleet, cockpitAuthority: cockpit.authority, cockpitCards: cockpit.cards.slice(0, 50) } }))
    }),
  FleetAttentionDecisionRequested: (payload: Readonly<{ action: FleetAttentionAction; interactionRef: string; runRef: string }>) =>
    Effect.gen(function* () {
      if (bridge.decideAttention === undefined) return
      const current = yield* SubscriptionRef.get(state)
      const card = current.fleet.cockpitCards.find(item => item.runRef === payload.runRef)
      if (card === undefined) return
      const command = admitFleetAttentionCommand(card, payload.interactionRef, payload.action)
      if (command === null) return
      yield* Effect.promise(() => bridge.decideAttention!(command).catch(() => null))
      if (bridge.cockpit === undefined) return
      const cockpit = yield* Effect.promise(() => bridge.cockpit!().catch(() => ({ authority: "unknown" as const, cards: [] })))
      yield* SubscriptionRef.update(state, next => ({ ...next, fleet: { ...next.fleet, cockpitAuthority: cockpit.authority, cockpitCards: cockpit.cards.slice(0, 50) } }))
    }),
})

// ---------------------------------------------------------------------------
// View — pure `state -> View` over the shared catalog.
// ---------------------------------------------------------------------------

const readinessTone = (readiness: FleetAccountReadiness): "success" | "warn" | "neutral" =>
  readiness === "ready" ? "success" : readiness === "credentials-missing" ? "warn" : "neutral"

/**
 * Local wall-clock rendering of an ISO instant ("11:08:44"). The instant
 * itself stays honest (it is when the projection/probe was decoded); only the
 * presentation moves from UTC to the operator's local time.
 */
export const formatFleetLocalTime = (iso: string, withSeconds = true): string => {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso.slice(0, 40)
  const hours = String(date.getHours()).padStart(2, "0")
  const minutes = String(date.getMinutes()).padStart(2, "0")
  return withSeconds ? `${hours}:${minutes}:${String(date.getSeconds()).padStart(2, "0")}` : `${hours}:${minutes}`
}

/** Deterministic display order: ready first, then provider, then ref. */
export const sortFleetAccounts = (
  accounts: ReadonlyArray<FleetAccount>,
): ReadonlyArray<FleetAccount> =>
  [...accounts].sort((left, right) =>
    (left.readiness === "ready" ? 0 : 1) - (right.readiness === "ready" ? 0 : 1) ||
    left.provider.localeCompare(right.provider) ||
    left.ref.localeCompare(right.ref))

/**
 * Readiness honesty (#8712 Lane C): the upstream projection's "ready" is
 * auth.json-PRESENCE-only — it does not prove the credential still works
 * (receipted: every registered codex home was "ready" while its refresh
 * token was revoked). Probe/child evidence therefore SUPERSEDES presence
 * evidence: a failed usage probe this session, or a Codex child that hit a
 * revoked credential (ledger `reconnectRequired`), overrides the account's
 * presence-based ready with a typed "reconnect required" state.
 */
export const fleetReconnectRequired = (
  fleet: FleetWorkspaceState,
  account: FleetAccount,
): boolean => {
  // Probe evidence rules in BOTH directions (EP250): a failed probe raises
  // the override, and a subsequent SUCCESSFUL probe this session clears a
  // stale ledger flag — after a UI reconnect the account is live again, and
  // the session ledger's sticky reconnectRequired must not outrank fresher
  // real-auth evidence. Usage entries are reset on every accounts refresh,
  // so a "checked" entry is always evidence from the current projection.
  const probeState = fleet.usage[account.ref]?.state
  if (probeState === "checked") return false
  if (probeState === "failed") return true
  return fleet.ledger?.rows.some((row) =>
    row.accountRef === account.ref && row.provider === account.provider &&
    row.reconnectRequired) ?? false
}

/** Lit ONLY on decoded ready evidence; everything else is explicitly not lit. */
export const fleetDotEvidence = (
  fleet: FleetWorkspaceState,
  account: FleetAccount,
): "lit" | "unlit" | "evidence-unavailable" | "reconnect-required" =>
  fleetReconnectRequired(fleet, account)
    ? "reconnect-required"
    : fleet.phase !== "ready" || account.readiness === "unknown"
      ? "evidence-unavailable"
      : account.readiness === "ready"
        ? "lit"
        : "unlit"

/**
 * One status chip: dot + account ref on one line with the provider small and
 * dim beside it. Chips stack vertically (a column flow) so the dots area can
 * never overflow the window horizontally — the catalog's closed Stack
 * contract has no wrapping row today (demand-register candidate), and a
 * clipped, horizontally scrolling dot strip would be worse than a list.
 */
const fleetStatusDot = (fleet: FleetWorkspaceState, account: FleetAccount): View => {
  const evidence = fleetDotEvidence(fleet, account)
  return Stack(
    {
      key: `fleet-dot-${account.ref}`,
      direction: "row",
      gap: "1",
      align: "center",
      style: { width: "full", minWidth: 0 },
    },
    [
      Icon({
        key: `fleet-dot-icon-${account.ref}`,
        name: "Circle",
        size: "sm",
        color: evidence === "lit" ? "success" : evidence === "reconnect-required" ? "warning" : "textMuted",
        label: `${account.ref} ${evidence === "lit" ? "ready" : evidence === "unlit" ? "credentials missing" : evidence === "reconnect-required" ? "reconnect required" : "evidence unavailable"}`,
      }),
      Text({
        key: `fleet-dot-label-${account.ref}`,
        content: account.ref,
        variant: "caption",
        color: "textPrimary",
      }),
      Text({
        key: `fleet-dot-provider-${account.ref}`,
        content: account.provider,
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
      ...(evidence === "reconnect-required"
        ? [Text({
            key: `fleet-dot-reconnect-${account.ref}`,
            content: "reconnect required",
            variant: "caption",
            color: "warning",
          })]
        : []),
    ],
  )
}

/**
 * The checked number is the exact usage of the minimal `--refresh` probe turn
 * (the pylon runbook: refresh "consumes a minimal provider call"), so it is
 * always labeled as the probe plus the local time it was refreshed — never a
 * bare unlabeled number.
 */
const usageSummaryLabel = (entry: Extract<FleetUsageEntry, { state: "checked" }>): string => {
  const time = formatFleetLocalTime(entry.refreshedAt, false)
  if (entry.totalTokens === null) return `no usage recorded · probe · ${time}`
  return [
    `${entry.totalTokens.toLocaleString("en-US")} tokens`,
    ...(entry.inputTokens !== null && entry.outputTokens !== null
      ? [`in ${entry.inputTokens.toLocaleString("en-US")} / out ${entry.outputTokens.toLocaleString("en-US")}`]
      : []),
    "probe",
    time,
  ].join(" · ")
}

const usageCell = (fleet: FleetWorkspaceState, account: FleetAccount): View => {
  const entry = fleet.usage[account.ref]
  const checking = entry?.state === "checking"
  // Same key in every phase so the control patches in place: while the usage
  // IPC call is pending the button is disabled and says "Checking…".
  const checkButton = Button({
    key: `fleet-usage-check-${account.ref}`,
    label: checking ? "Checking…" : "Check",
    variant: "ghost",
    disabled: checking,
    onPress: IntentRef("FleetUsageCheckRequested", StaticPayload(account.ref)),
    a11y: {
      label: checking
        ? `Checking usage for account ${account.ref}`
        : `Check usage for account ${account.ref}`,
    },
  })
  if (entry === undefined || checking) {
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
        content: usageSummaryLabel(entry),
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
      { id: "usage", header: "Usage", align: "end" },
    ],
    rows: sortFleetAccounts(fleet.accounts).map((account) => ({
      id: account.ref,
      cells: [
        Text({ key: `fleet-provider-${account.ref}`, content: account.provider, variant: "caption", color: "textMuted" }),
        Text({ key: `fleet-ref-${account.ref}`, content: account.ref, variant: "body", color: "textPrimary" }),
        Text({ key: `fleet-email-${account.ref}`, content: account.email ?? "—", variant: "caption", color: "textMuted" }),
        // Readiness honesty: a session-observed revoked credential or failed
        // probe supersedes the projection's presence-based value in the cell.
        // Broken-credential rows carry a "Fix in Settings" navigation (EP250:
        // Fleet stays overview-only — account MANAGEMENT lives in Settings,
        // reached through the existing DesktopSettingsToggled intent; no
        // account mutation happens from this view).
        fleetReconnectRequired(fleet, account) || account.readiness === "credentials-missing"
          ? Stack(
              {
                key: `fleet-readiness-cell-${account.ref}`,
                direction: "row",
                gap: "1",
                align: "center",
              },
              [
                fleetReconnectRequired(fleet, account)
                  ? Badge({
                      key: `fleet-readiness-${account.ref}`,
                      label: "reconnect required",
                      tone: "warn",
                      a11y: {
                        label: `Account ${account.ref} readiness: reconnect required (probe evidence supersedes presence-based ${account.readiness})`,
                      },
                    })
                  : Badge({
                      key: `fleet-readiness-${account.ref}`,
                      label: account.readiness,
                      tone: readinessTone(account.readiness),
                      a11y: { label: `Account ${account.ref} readiness: ${account.readiness}` },
                    }),
                Button({
                  key: `fleet-fix-${account.ref}`,
                  label: "Fix in Settings",
                  variant: "ghost",
                  onPress: IntentRef("DesktopSettingsToggled"),
                  a11y: {
                    label: `Fix account ${account.ref} in Settings (reconnect runs there)`,
                  },
                }),
              ],
            )
          : Badge({
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

/**
 * Session usage (#8712 Lane C): a compact evidence-labeled section fed from
 * main's session ledger — exact tokens this desktop session dispatched
 * (Fable turns + Codex delegate children), per account. Codex rows show the
 * requested model (spawn-config truth: gpt-5.6-sol). Labeled "session
 * ledger" so it is never confused with the per-account "probe" numbers in
 * the table above.
 */
const sessionUsageRow = (row: UsageLedgerRow): View =>
  Stack(
    {
      key: `fleet-ledger-row-${row.accountRef}`,
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full", minWidth: 0 },
    },
    [
      Text({
        key: `fleet-ledger-ref-${row.accountRef}`,
        content: row.accountRef,
        variant: "caption",
        color: "textPrimary",
      }),
      Text({
        key: `fleet-ledger-provider-${row.accountRef}`,
        content: row.provider,
        variant: "caption",
        color: "textMuted",
      }),
      ...(row.requestedModel === null
        ? []
        : [Text({
            key: `fleet-ledger-model-${row.accountRef}`,
            content: `${row.requestedModel} (requested)`,
            variant: "caption",
            color: "textMuted",
          })]),
      Text({
        key: `fleet-ledger-counts-${row.accountRef}`,
        content: `${row.turns} turn(s) · ${row.children} child(ren)`,
        variant: "caption",
        color: "textMuted",
      }),
      Text({
        key: `fleet-ledger-total-${row.accountRef}`,
        content: `${row.totalTokens.toLocaleString("en-US")} tokens`,
        variant: "caption",
        color: "textPrimary",
      }),
      ...(row.reconnectRequired
        ? [Text({
            key: `fleet-ledger-reconnect-${row.accountRef}`,
            content: "reconnect required",
            variant: "caption",
            color: "warning",
          })]
        : []),
    ],
  )

const sessionUsageSection = (fleet: FleetWorkspaceState): ReadonlyArray<View> => {
  if (fleet.ledger === null || fleet.ledger.rows.length === 0) return []
  return [
    Stack(
      {
        key: "fleet-session-usage",
        direction: "column",
        gap: "1",
        style: { width: "full", minWidth: 0 },
      },
      [
        Stack({ key: "fleet-session-usage-heading", direction: "row", gap: "2", align: "center" }, [
          Text({
            key: "fleet-session-usage-title",
            content: "Session usage",
            variant: "label",
            color: "textMuted",
          }),
          Text({
            key: "fleet-session-usage-evidence",
            content: `${fleet.ledger.evidence} · as of ${formatFleetLocalTime(fleet.ledger.generatedAt)}`,
            variant: "caption",
            color: "textMuted",
          }),
        ]),
        ...fleet.ledger.rows.map(sessionUsageRow),
      ],
    ),
  ]
}

const fleetBody = (fleet: FleetWorkspaceState): ReadonlyArray<View> => {
  const authorityRows: ReadonlyArray<View> = (fleet.authorityRuns?.runs ?? []).flatMap(run => [
    Text({ key: `authority-run-${run.runRef}`, content: `Fleet run · ${run.executionState}\n${run.runRef}`, variant: "label", color: "textPrimary" }),
    ...run.attempts.map(attempt => Text({
      key: `authority-attempt-${attempt.workClaimRef}`,
      content: `${attempt.requestedTarget} → ${attempt.selectedTarget} · ${attempt.outcome}\n${attempt.workClaimRef}\n${attempt.assignmentRef ?? "Assignment pending"}\n${attempt.closeoutRef ?? "Closeout pending"}`,
      variant: "caption",
      color: "textMuted",
    })),
  ])
  if (fleet.phase === "loading" || fleet.phase === "idle") {
    return [Text({
      key: "fleet-loading",
      content: "Loading provider accounts…",
      variant: "body",
      color: "textMuted",
    })]
  }
  if (fleet.phase === "unavailable") {
    return [...authorityRows,
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
    return [...authorityRows, ...fleetCockpitSection(fleet), Text({
      key: "fleet-empty",
      content: "No provider accounts connected yet. Connect one in Settings.",
      variant: "body",
      color: "textMuted",
    })]
  }
  return [
    ...authorityRows,
    ...fleetCockpitSection(fleet),
    // Column flow (not a row): chips can never run off the right window edge.
    Stack(
      { key: "fleet-status-dots", direction: "column", gap: "1", style: { width: "full", minWidth: 0 } },
      sortFleetAccounts(fleet.accounts).map((account) => fleetStatusDot(fleet, account)),
    ),
    fleetAccountsTable(fleet),
    ...sessionUsageSection(fleet),
  ]
}

const fleetCockpitSection = (fleet: FleetWorkspaceState): ReadonlyArray<View> => {
  if (fleet.cockpitCards.length === 0) {
    return [Text({ key: "fleet-cockpit-empty", content: fleet.cockpitAuthority === "live" ? "No active or recent canonical runs." : `Run authority ${fleet.cockpitAuthority}; controls withheld.`, variant: "body", color: "textMuted" })]
  }
  return [
    Text({ key: "fleet-cockpit-title", content: "Authoritative work", variant: "label", color: "textMuted" }),
    ...fleet.cockpitCards.map(card => Stack(
      { key: `fleet-cockpit-${card.runRef}`, direction: "column", gap: "1", style: { width: "full", backgroundColor: "surfaceRaised", borderRadius: "md", padding: "2" } },
      [
        Stack({ key: `fleet-cockpit-${card.runRef}-head`, direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
          Text({ key: `fleet-cockpit-${card.runRef}-title`, content: card.title, variant: "body", color: "textPrimary" }),
          Badge({ key: `fleet-cockpit-${card.runRef}-provider`, label: card.provider, tone: "neutral", a11y: { label: `Provider ${card.provider}` } }),
          Badge({ key: `fleet-cockpit-${card.runRef}-status`, label: card.status, tone: card.status === "running" ? "success" : card.status === "failed" ? "warn" : "neutral", a11y: { label: `Run ${card.status}` } }),
          Spacer({ key: `fleet-cockpit-${card.runRef}-fill`, flex: true }),
          Button({ key: `fleet-cockpit-${card.runRef}-open`, label: "Open", variant: "ghost", onPress: IntentRef("DesktopChatSelected", StaticPayload(card.threadRef)), a11y: { label: `Open conversation ${card.title}` } }),
        ]),
        Text({ key: `fleet-cockpit-${card.runRef}-refs`, content: [card.workContextRef, card.repositoryRef, ...card.agentRefs, ...card.receiptRefs].filter((value): value is string => value !== null).join(" · "), variant: "caption", color: "textMuted" }),
        ...(card.attention.length === 0 ? [] : [
          Text({ key: `fleet-cockpit-${card.runRef}-attention`, content: `${card.attention.length} item${card.attention.length === 1 ? "" : "s"} ${card.attention.length === 1 ? "needs" : "need"} attention`, variant: "label", color: "warning" }),
          ...card.attention.map(attention => Stack({ key: `fleet-cockpit-${card.runRef}-${attention.interactionRef}`, direction: "row", gap: "2", align: "center" }, [
            Text({ key: `fleet-cockpit-${card.runRef}-${attention.interactionRef}-title`, content: attention.title, variant: "body", color: "textPrimary" }),
            Spacer({ key: `fleet-cockpit-${card.runRef}-${attention.interactionRef}-fill`, flex: true }),
            ...attention.actions.map(action => Button({
              key: `fleet-cockpit-${card.runRef}-${attention.interactionRef}-${action}`,
              label: action === "approve" ? "Approve" : "Deny",
              variant: action === "approve" ? "primary" : "secondary",
              onPress: IntentRef("FleetAttentionDecisionRequested", StaticPayload({ action, interactionRef: attention.interactionRef, runRef: card.runRef })),
              a11y: { label: `${action} ${attention.title}` },
            })),
          ])),
        ]),
        ...(card.actions.length === 0 ? [] : [Stack({ key: `fleet-cockpit-${card.runRef}-controls`, direction: "row", gap: "2", align: "center" }, card.actions.map(action => Button({
          key: `fleet-cockpit-${card.runRef}-${action}`,
          label: action[0]!.toUpperCase() + action.slice(1),
          variant: action === "resume" ? "primary" : action === "close" ? "ghost" : "secondary",
          onPress: IntentRef("FleetRunControlRequested", StaticPayload({ action, runRef: card.runRef })),
          a11y: { label: `${action} ${card.title}` },
        })))]),
      ],
    )),
  ]
}

export const fleetWorkspaceView = (fleet: FleetWorkspaceState): View =>
  Stack(
    {
      key: "workspace-fleet-panel",
      direction: "column",
      gap: "3",
      // Right padding keeps the last table column and the Refresh control off
      // the window edge (the "Usage" header was clipping at full width).
      style: { width: "full", minWidth: 0, flex: 1, minHeight: 0, paddingRight: "4", paddingTop: "2" },
    },
    [
      Stack({ key: "fleet-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "fleet-title", content: "Fleet", variant: "heading", color: "textPrimary" }),
        Text({
          key: "fleet-as-of",
          content: fleet.generatedAt === null
            ? "no snapshot yet"
            : `as of ${formatFleetLocalTime(fleet.generatedAt)}`,
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
