import { Option, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonUnknown } from './json-boundary'
import type { NexusPaymentAuthorityReceiptRecord } from './nexus-treasury-payout-ledger'
import { NexusTreasuryPayoutAdapterKind } from './nexus-treasury-payout-ledger'
import { TassadarRunSettlementHardPerPayoutCapSats } from './tassadar-run-settlement'

/**
 * Owner gate for REAL Bitcoin settlement (openagents #5232, Gate 2).
 *
 * Default behavior is byte-for-byte the current simulation. The real-money
 * branch is reachable ONLY when an owner explicitly enables this gate AND every
 * bound is satisfied: a small per-payout sat cap, one allowed recipient, one
 * allowed run, and an explicit real adapter request. Anything missing,
 * unparseable, or out of bounds fails CLOSED to `simulation`.
 *
 * This module is a pure decision surface. It does not move money, dispatch, or
 * read wallets. The route only ever simulates until an owner both sets this
 * gate and (separately) wires the live dispatch path — see
 * docs/2026-06-17-real-settlement-gate2-design.md §6.
 */

// The only real adapter this gate may authorize. The proven Spark treasury
// payout rail (#5176/#5185/#5196/#5208) is the single allowed real adapter.
export const TassadarRealSettlementAllowedAdapterKind = S.Literals([
  'spark_treasury',
])
export type TassadarRealSettlementAllowedAdapterKind =
  typeof TassadarRealSettlementAllowedAdapterKind.Type

export const TassadarRealSettlementGateEnvKey =
  'OPENAGENTS_REAL_SETTLEMENT_GATE'

// Defense-in-depth ceiling on the cumulative daily real-settled budget,
// independent of any owner-set value. Hands-off auto-streaming (openagents
// #5309) must never let a misconfigured gate authorize more than this per UTC
// day. The owner sets a smaller value (e.g. 50_000); this is the absolute roof.
export const TassadarRunSettlementHardDailyCapSats = 1_000_000

export const TassadarRealSettlementGate = S.Struct({
  // Explicit owner enable. Default OFF: a gate with `enabled: false` (or an
  // absent/malformed env value) keeps every settlement on `simulation`.
  enabled: S.Boolean,
  // The single real adapter the gate authorizes. Must equal the adapter the
  // caller explicitly requests before the real branch is reachable.
  allowedAdapterKind: TassadarRealSettlementAllowedAdapterKind,
  // Recipients (contributor pylon refs) explicitly allowed to receive real
  // sats. With `runScopedStreaming` off/absent, a recipient must appear here.
  allowedContributorRefs: S.Array(S.String).check(S.isMaxLength(64)),
  // Workload families (training run refs) allowed to settle for real.
  allowedRunRefs: S.Array(S.String).check(S.isMaxLength(8)),
  // Hard per-payout sat cap, itself clamped to the module hard ceiling below.
  maxPayoutSats: S.Number.check(
    S.isInt(),
    S.isBetween({
      maximum: TassadarRunSettlementHardPerPayoutCapSats,
      minimum: 1,
    }),
  ),
  // OPTIONAL cumulative daily real-settled budget (openagents #5309). When
  // ABSENT the gate keeps its prior per-payout-only behavior byte-for-byte (no
  // aggregate ceiling), so an already-armed gate value decodes and behaves
  // exactly as before. When PRESENT it is the maximum real sats that may be
  // auto-settled per UTC day; once a day's real total would exceed it, further
  // auto-settlements fall back to simulation/skip until the next UTC day. Itself
  // clamped to the module hard daily ceiling above.
  maxDailyPayoutSats: S.optionalKey(
    S.Number.check(
      S.isInt(),
      S.isBetween({
        maximum: TassadarRunSettlementHardDailyCapSats,
        minimum: 1,
      }),
    ),
  ),
  // OPTIONAL run-scoped streaming eligibility (openagents #5309/#5310). When
  // ABSENT or false, only `allowedContributorRefs` recipients are eligible (the
  // prior behavior, unchanged). When true, ANY contributor with a registered
  // Spark payout target on an allowlisted run is eligible — still bounded by the
  // per-payout cap, the daily cap, and the independent worker!=validator replay
  // verification that produced the Verified pair. The explicit
  // `allowedContributorRefs` path keeps working alongside it.
  runScopedStreaming: S.optionalKey(S.Boolean),
})
export type TassadarRealSettlementGate =
  typeof TassadarRealSettlementGate.Type

/**
 * The disabled gate. Any absent / malformed / partial config resolves to this,
 * so the real branch is never reachable by accident.
 */
export const disabledTassadarRealSettlementGate: TassadarRealSettlementGate = {
  enabled: false,
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: [],
  maxPayoutSats: 1,
}

const decodeGate = S.decodeUnknownOption(TassadarRealSettlementGate)

/**
 * Parse the owner gate from a raw env string at the boundary (no ad hoc
 * `JSON.parse`; routes through `parseJsonUnknown` + a typed Schema decode).
 * Fails CLOSED: any error, absence, or `enabled !== true` yields the disabled
 * gate.
 */
export const parseTassadarRealSettlementGate = (
  rawValue: string | undefined,
): TassadarRealSettlementGate => {
  if (rawValue === undefined || rawValue.trim() === '') {
    return disabledTassadarRealSettlementGate
  }

  const parsed = (() => {
    try {
      return parseJsonUnknown(rawValue)
    } catch {
      return undefined
    }
  })()

  if (parsed === undefined) {
    return disabledTassadarRealSettlementGate
  }

  return Option.match(decodeGate(parsed), {
    onNone: () => disabledTassadarRealSettlementGate,
    onSome: gate =>
      gate.enabled ? gate : disabledTassadarRealSettlementGate,
  })
}

/**
 * Read the owner gate from the worker env. The env value is the JSON gate
 * config under `OPENAGENTS_REAL_SETTLEMENT_GATE`. Anything else is disabled.
 */
export const readTassadarRealSettlementGate = (
  env: Readonly<Record<string, unknown>>,
): TassadarRealSettlementGate => {
  const rawValue = env[TassadarRealSettlementGateEnvKey]

  return parseTassadarRealSettlementGate(
    typeof rawValue === 'string' ? rawValue : undefined,
  )
}

export type TassadarSettlementAdapterDecisionInput = Readonly<{
  amountSats: number
  contributorRef: string
  gate: TassadarRealSettlementGate
  requestedAdapterKind: typeof NexusTreasuryPayoutAdapterKind.Type
  trainingRunRef: string
}>

export type TassadarSettlementAdapterDecision = Readonly<{
  adapterKind: typeof NexusTreasuryPayoutAdapterKind.Type
  blockedReason:
    | 'amount_over_gate_cap'
    | 'contributor_not_allowlisted'
    | 'gate_disabled'
    | 'requested_adapter_mismatch'
    | 'run_not_allowlisted'
    | null
  // How the recipient qualified for the real branch: `allowlisted` means the
  // contributor appeared in `allowedContributorRefs`; `run_scoped_streaming`
  // means it qualified via `runScopedStreaming` on an allowlisted run (still
  // bounded by per-payout cap, daily cap, and the verification requirement);
  // `null` when the real branch was not authorized.
  eligibilitySource: 'allowlisted' | 'run_scoped_streaming' | null
  realAuthorized: boolean
}>

/**
 * Resolve the settlement adapter. Returns `simulation` unless the gate is
 * enabled AND every bound holds: the caller explicitly requested the gate's
 * allowed real adapter, the amount is within the gate cap, and the contributor
 * and run are both allowlisted. Each failing condition surfaces a typed
 * `blockedReason` for operator logs (never a raw value). Fails CLOSED.
 */
export const resolveTassadarSettlementAdapter = (
  input: TassadarSettlementAdapterDecisionInput,
): TassadarSettlementAdapterDecision => {
  const simulation: TassadarSettlementAdapterDecision = {
    adapterKind: 'simulation',
    blockedReason: null,
    eligibilitySource: null,
    realAuthorized: false,
  }

  if (!input.gate.enabled) {
    return { ...simulation, blockedReason: 'gate_disabled' }
  }

  if (input.requestedAdapterKind !== input.gate.allowedAdapterKind) {
    return { ...simulation, blockedReason: 'requested_adapter_mismatch' }
  }

  if (input.amountSats > input.gate.maxPayoutSats) {
    return { ...simulation, blockedReason: 'amount_over_gate_cap' }
  }

  // The run must always be allowlisted, even under run-scoped streaming: the
  // run allowlist is the operator's enrollment boundary for which workloads may
  // stream real sats at all.
  if (!input.gate.allowedRunRefs.includes(input.trainingRunRef)) {
    return { ...simulation, blockedReason: 'run_not_allowlisted' }
  }

  // Eligibility: an explicitly allowlisted contributor always qualifies. With
  // run-scoped streaming on, ANY contributor on this allowlisted run qualifies
  // (the registered-payout-target requirement is enforced downstream by the
  // destination resolver, which fails closed/skips when absent). The trust
  // anchor remains the independent worker!=validator replay verification.
  const allowlisted = input.gate.allowedContributorRefs.includes(
    input.contributorRef,
  )
  const eligibilitySource: 'allowlisted' | 'run_scoped_streaming' | null =
    allowlisted
      ? 'allowlisted'
      : input.gate.runScopedStreaming === true
        ? 'run_scoped_streaming'
        : null

  if (eligibilitySource === null) {
    return { ...simulation, blockedReason: 'contributor_not_allowlisted' }
  }

  return {
    adapterKind: input.gate.allowedAdapterKind,
    blockedReason: null,
    eligibilitySource,
    realAuthorized: true,
  }
}

/**
 * The cumulative daily real-settled budget (openagents #5309) is tracked durably
 * by SUMMING existing `settlement_recorded` receipts that prove real bitcoin
 * movement. There is no separate counter to drift out of sync: the receipt
 * ledger is the source of truth, so the daily total is receipt-first by
 * construction.
 *
 * The daily window is keyed by the UTC calendar day (`YYYY-MM-DD` from the
 * receipt `createdAt`). UTC — not a local zone — is the deliberate boundary so
 * the reset is deterministic across deployments and operator locations; an
 * auto-settle at 23:59:59Z counts against that day and the budget resets at
 * 00:00:00Z the next UTC day.
 */
export const tassadarRealSettlementUtcDayKey = (iso: string): string =>
  iso.slice(0, 10)

const realSettledSatsFromReceipt = (
  record: Pick<
    NexusPaymentAuthorityReceiptRecord,
    'publicProjectionJson' | 'receiptKind'
  >,
): number => {
  if (record.receiptKind !== 'settlement_recorded') {
    return 0
  }

  const projection = parseJsonRecord(record.publicProjectionJson)

  if (projection === undefined) {
    return 0
  }

  // Only REAL bitcoin movement counts against the daily budget. Simulation
  // receipts (moneyMovement:'none') and any non-settled state are ignored, so
  // proofs/tests that record simulation chains never consume real budget.
  const amountSats = projection.amountSats

  return projection.state === 'settled' &&
    projection.moneyMovement === 'real_bitcoin' &&
    typeof amountSats === 'number' &&
    Number.isInteger(amountSats) &&
    amountSats > 0
    ? amountSats
    : 0
}

/**
 * Sum the real-settled sats recorded on the given UTC day across a list of
 * settlement receipts. Pure; the caller supplies the receipt list (read from
 * the ledger) and the day key for "now".
 */
export const tassadarRealSettledSatsForDay = (
  receipts: ReadonlyArray<
    Pick<
      NexusPaymentAuthorityReceiptRecord,
      'createdAt' | 'publicProjectionJson' | 'receiptKind'
    >
  >,
  utcDayKey: string,
): number =>
  receipts.reduce(
    (total, receipt) =>
      tassadarRealSettlementUtcDayKey(receipt.createdAt) === utcDayKey
        ? total + realSettledSatsFromReceipt(receipt)
        : total,
    0,
  )

export type TassadarDailyBudgetDecision = Readonly<{
  // Sats already settled for real on this UTC day, before this payout.
  alreadySettledTodaySats: number
  // Whether this payout of `amountSats` fits under the daily cap.
  authorized: boolean
  // The effective daily cap in force (the gate value clamped to the hard roof),
  // or null when the gate declares no daily cap (per-payout-only mode).
  effectiveDailyCapSats: number | null
  // Public-safe remaining budget for observability AFTER reserving this payout
  // when authorized, or the current remaining when blocked. Null in
  // per-payout-only mode (no aggregate ceiling to report).
  remainingDailyBudgetSats: number | null
}>

/**
 * Decide whether one auto-settlement of `amountSats` may proceed under the
 * cumulative daily budget (openagents #5309). Fails CLOSED: when a daily cap is
 * present and this payout would push the day's real total over it, the payout
 * is NOT authorized (the caller falls back to simulation/skip). When the gate
 * declares no `maxDailyPayoutSats`, this preserves the prior per-payout-only
 * behavior (always authorized at this layer; the per-payout cap still applies
 * elsewhere). Pure.
 */
export const decideTassadarDailyBudget = (
  input: Readonly<{
    alreadySettledTodaySats: number
    amountSats: number
    gate: TassadarRealSettlementGate
  }>,
): TassadarDailyBudgetDecision => {
  const declaredCap = input.gate.maxDailyPayoutSats

  if (declaredCap === undefined) {
    return {
      alreadySettledTodaySats: input.alreadySettledTodaySats,
      authorized: true,
      effectiveDailyCapSats: null,
      remainingDailyBudgetSats: null,
    }
  }

  const effectiveDailyCapSats = Math.min(
    declaredCap,
    TassadarRunSettlementHardDailyCapSats,
  )
  const projectedTotal = input.alreadySettledTodaySats + input.amountSats
  const authorized = projectedTotal <= effectiveDailyCapSats
  const remainingDailyBudgetSats = Math.max(
    0,
    effectiveDailyCapSats -
      (authorized ? projectedTotal : input.alreadySettledTodaySats),
  )

  return {
    alreadySettledTodaySats: input.alreadySettledTodaySats,
    authorized,
    effectiveDailyCapSats,
    remainingDailyBudgetSats,
  }
}
