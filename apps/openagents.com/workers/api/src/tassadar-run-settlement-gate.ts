import { Option, Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'
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

export const TassadarRealSettlementGate = S.Struct({
  // Explicit owner enable. Default OFF: a gate with `enabled: false` (or an
  // absent/malformed env value) keeps every settlement on `simulation`.
  enabled: S.Boolean,
  // The single real adapter the gate authorizes. Must equal the adapter the
  // caller explicitly requests before the real branch is reachable.
  allowedAdapterKind: TassadarRealSettlementAllowedAdapterKind,
  // One recipient (contributor pylon ref) allowed to receive real sats.
  allowedContributorRefs: S.Array(S.String).check(S.isMaxLength(8)),
  // One workload family (training run ref) allowed to settle for real.
  allowedRunRefs: S.Array(S.String).check(S.isMaxLength(8)),
  // Hard per-payout sat cap, itself clamped to the module hard ceiling below.
  maxPayoutSats: S.Number.check(
    S.isInt(),
    S.isBetween({
      maximum: TassadarRunSettlementHardPerPayoutCapSats,
      minimum: 1,
    }),
  ),
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

  if (!input.gate.allowedContributorRefs.includes(input.contributorRef)) {
    return { ...simulation, blockedReason: 'contributor_not_allowlisted' }
  }

  if (!input.gate.allowedRunRefs.includes(input.trainingRunRef)) {
    return { ...simulation, blockedReason: 'run_not_allowlisted' }
  }

  return {
    adapterKind: input.gate.allowedAdapterKind,
    blockedReason: null,
    realAuthorized: true,
  }
}
