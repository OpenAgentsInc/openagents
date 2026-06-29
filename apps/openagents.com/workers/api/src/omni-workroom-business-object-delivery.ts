import { Schema as S } from 'effect'

import type { OmniProjectionAudience } from './omni-data-classification'
import type { OmniWorkroomRecord } from './omni-workrooms'
import {
  type OmniBusinessObjectWriteRecord,
  type OmniSourceAuthorityBinding,
  OmniBusinessObjectWriteRecord as OmniBusinessObjectWriteRecordSchema,
  OmniSourceAuthorityBinding as OmniSourceAuthorityBindingSchema,
  decideOmniBusinessObjectWrite,
  projectOmniBusinessObjectWrite,
} from './omni-source-authorized-business-objects'

// ---------------------------------------------------------------------------
// Workroom-delivery integration for source-authorized business objects
// (DE-9 / EPIC #5532).
//
// This is the seam where the source-authority + approval-gated write model
// touches the LIVE omni client-delivery workroom surface
// (workrooms.omni_client_delivery_workrooms.v1, yellow). By default the
// integration is disabled. When the owner-gated delivery config reaches
// enabled_ready, approved proposed writes are materialized as applied
// business-object projections with deterministic write and closeout receipts.
// The seam still never sends, settles, spends, mutates connectors, or upgrades
// a public claim by itself.
//
// Promotion of the source-authorized-business-objects promise to green
// requires the live integration ENABLED, an owner sign-off ref, and a
// closeout receipt — which is owner-gated and out of scope for this build.
// ---------------------------------------------------------------------------

/**
 * The integration gate state. `inert_disabled` is the default and the only
 * state this build ships in. The remaining states describe the gate ladder a
 * future owner-armed promotion walks; none of them are flipped here.
 */
export const OmniBusinessObjectDeliveryGateState = S.Literals([
  'inert_disabled',
  'enabled_blocked',
  'enabled_ready',
])
export type OmniBusinessObjectDeliveryGateState =
  typeof OmniBusinessObjectDeliveryGateState.Type

/**
 * Operator/owner configuration for the integration. Defaults keep every live
 * effect off. `integrationEnabled` is the master INERT flag; `ownerSignOffRef`
 * and `closeoutReceiptRef` are the additional gates required before the gate
 * may report `enabled_ready`.
 */
export type OmniBusinessObjectDeliveryConfig = Readonly<{
  closeoutReceiptRef?: string | undefined
  integrationEnabled?: boolean | undefined
  ownerSignOffRef?: string | undefined
}>

export const OMNI_BUSINESS_OBJECT_DELIVERY_INERT_CONFIG: OmniBusinessObjectDeliveryConfig =
  {
    closeoutReceiptRef: undefined,
    integrationEnabled: false,
    ownerSignOffRef: undefined,
  }

/**
 * A single planned write entry: the write projection plus the pure
 * approval-gated decision for it. `applyAllowed` here reflects whether the
 * write COULD be applied under the source-authority model; it is still held
 * inert by the integration gate unless the gate reaches `enabled_ready`.
 */
export class OmniBusinessObjectDeliveryPlanEntry extends S.Class<OmniBusinessObjectDeliveryPlanEntry>(
  'OmniBusinessObjectDeliveryPlanEntry',
)({
  appliedBusinessObject: S.optionalKey(S.Unknown),
  appliedReceiptRefs: S.Array(S.String),
  applyAllowed: S.Boolean,
  approvalRequired: S.Boolean,
  blockerRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  reasonRef: S.String,
  write: S.Unknown,
}) {}

export class OmniAppliedBusinessObjectProjection extends S.Class<OmniAppliedBusinessObjectProjection>(
  'OmniAppliedBusinessObjectProjection',
)({
  appliedAtIso: S.String,
  appliedReceiptRefs: S.Array(S.String),
  businessObjectKind: S.String,
  businessObjectRef: S.String,
  closeoutReceiptRefs: S.Array(S.String),
  operation: S.String,
  sourceRefs: S.Array(S.String),
  writeRef: S.String,
}) {}

export class OmniBusinessObjectDeliveryPlan extends S.Class<OmniBusinessObjectDeliveryPlan>(
  'OmniBusinessObjectDeliveryPlan',
)({
  applyableCount: S.Number,
  audience: S.String,
  blockerRefs: S.Array(S.String),
  // The integration never applies writes itself; this is always false in this
  // build. It exists so callers cannot mistake a plan for an applied effect.
  effectsApplied: S.Boolean,
  entries: S.Array(OmniBusinessObjectDeliveryPlanEntry),
  gateState: OmniBusinessObjectDeliveryGateState,
  proposedCount: S.Number,
  workroomId: S.String,
}) {}

export class OmniBusinessObjectDeliveryUnsafe extends S.TaggedErrorClass<OmniBusinessObjectDeliveryUnsafe>()(
  'OmniBusinessObjectDeliveryUnsafe',
  { reason: S.String },
) {}

/**
 * Resolve the integration gate state from config. `inert_disabled` unless the
 * master flag is on; `enabled_ready` only when the flag is on AND an owner
 * sign-off ref AND a closeout receipt ref are present; otherwise
 * `enabled_blocked`.
 */
export const resolveOmniBusinessObjectDeliveryGate = (
  config: OmniBusinessObjectDeliveryConfig,
): OmniBusinessObjectDeliveryGateState => {
  if (config.integrationEnabled !== true) {
    return 'inert_disabled'
  }

  const hasOwnerSignOff =
    typeof config.ownerSignOffRef === 'string' &&
    config.ownerSignOffRef.trim() !== ''
  const hasCloseoutReceipt =
    typeof config.closeoutReceiptRef === 'string' &&
    config.closeoutReceiptRef.trim() !== ''

  return hasOwnerSignOff && hasCloseoutReceipt
    ? 'enabled_ready'
    : 'enabled_blocked'
}

const gateBlockerRefs = (
  gateState: OmniBusinessObjectDeliveryGateState,
  config: OmniBusinessObjectDeliveryConfig,
): ReadonlyArray<string> => {
  if (gateState === 'inert_disabled') {
    return ['blocker.business_object_delivery.integration_inert_disabled']
  }

  if (gateState === 'enabled_ready') {
    return []
  }

  const blockers: Array<string> = []

  if (
    typeof config.ownerSignOffRef !== 'string' ||
    config.ownerSignOffRef.trim() === ''
  ) {
    blockers.push('blocker.business_object_delivery.owner_sign_off_missing')
  }

  if (
    typeof config.closeoutReceiptRef !== 'string' ||
    config.closeoutReceiptRef.trim() === ''
  ) {
    blockers.push('blocker.business_object_delivery.closeout_receipt_missing')
  }

  return blockers.sort()
}

const deterministicApplyReceiptRefs = (
  record: OmniBusinessObjectWriteRecord,
  config: OmniBusinessObjectDeliveryConfig,
): ReadonlyArray<string> => [
  ...(record.appliedReceiptRefs.length > 0
    ? record.appliedReceiptRefs
    : [`receipt.business_object_delivery.${record.id}.applied`]),
  ...(typeof config.closeoutReceiptRef === 'string' &&
  config.closeoutReceiptRef.trim() !== ''
    ? [config.closeoutReceiptRef.trim()]
    : []),
]

const deterministicCloseoutReceiptRefs = (
  record: OmniBusinessObjectWriteRecord,
  config: OmniBusinessObjectDeliveryConfig,
): ReadonlyArray<string> => [
  ...(record.closeoutRefs.length > 0
    ? record.closeoutRefs
    : [`closeout.business_object_delivery.${record.id}.applied`]),
  ...(typeof config.closeoutReceiptRef === 'string' &&
  config.closeoutReceiptRef.trim() !== ''
    ? [config.closeoutReceiptRef.trim()]
    : []),
]

const appliedBusinessObjectForWrite = (
  record: OmniBusinessObjectWriteRecord,
  config: OmniBusinessObjectDeliveryConfig,
  nowIso: string,
): OmniAppliedBusinessObjectProjection =>
  new OmniAppliedBusinessObjectProjection({
    appliedAtIso: nowIso,
    appliedReceiptRefs: [
      ...new Set(deterministicApplyReceiptRefs(record, config)),
    ].sort(),
    businessObjectKind: record.businessObjectKind,
    businessObjectRef: record.businessObjectRef,
    closeoutReceiptRefs: [
      ...new Set(deterministicCloseoutReceiptRefs(record, config)),
    ].sort(),
    operation: record.operation,
    sourceRefs: [...new Set(record.sourceRefs)].sort(),
    writeRef: record.id,
  })

const blockedEntryForUnsafeWrite = (
  input: Readonly<{
    reason: string
    write: OmniBusinessObjectWriteRecord
  }>,
): OmniBusinessObjectDeliveryPlanEntry =>
  new OmniBusinessObjectDeliveryPlanEntry({
    appliedBusinessObject: undefined,
    appliedReceiptRefs: [],
    applyAllowed: false,
    approvalRequired: true,
    blockerRefs: ['blocker.business_object_delivery.write_unsafe'],
    closeoutReceiptRefs: [],
    reasonRef: 'reason.business_object_delivery.write_unsafe',
    write: {
      id: input.write.id,
      blockerRefs: ['blocker.business_object_delivery.write_unsafe'],
      reason: input.reason,
      state: 'blocked',
    },
  })

/**
 * Build the inert delivery plan for a live client-delivery workroom. It
 * matches each write to its named binding, runs the pure approval-gated
 * decision, and returns a plan plus the integration gate verdict. It applies
 * nothing. `effectsApplied` is always false.
 */
export const buildOmniBusinessObjectDeliveryPlan = (
  input: Readonly<{
    audience: OmniProjectionAudience
    bindings: ReadonlyArray<OmniSourceAuthorityBinding>
    config?: OmniBusinessObjectDeliveryConfig | undefined
    nowIso: string
    workroom: OmniWorkroomRecord
    writes: ReadonlyArray<OmniBusinessObjectWriteRecord>
  }>,
): OmniBusinessObjectDeliveryPlan => {
  const config = input.config ?? OMNI_BUSINESS_OBJECT_DELIVERY_INERT_CONFIG
  const gateState = resolveOmniBusinessObjectDeliveryGate(config)

  const bindingsById = new Map<string, OmniSourceAuthorityBinding>(
    input.bindings.map(binding => [binding.id, binding]),
  )

  const entries: Array<OmniBusinessObjectDeliveryPlanEntry> = input.writes.map(
    write => {
      const binding = bindingsById.get(write.bindingRef)

      if (binding === undefined) {
        return new OmniBusinessObjectDeliveryPlanEntry({
          appliedBusinessObject: undefined,
          appliedReceiptRefs: [],
          applyAllowed: false,
          approvalRequired: true,
          blockerRefs: ['blocker.business_object_delivery.binding_not_found'],
          closeoutReceiptRefs: [],
          reasonRef: 'reason.business_object_delivery.binding_not_found',
          write: projectOmniBusinessObjectWrite(
            write,
            input.audience,
            input.nowIso,
          ),
        })
      }

      let decision
      try {
        decision = decideOmniBusinessObjectWrite(binding, write)
      } catch (error) {
        return blockedEntryForUnsafeWrite({
          reason: error instanceof Error ? error.message : String(error),
          write,
        })
      }

      const integrationApplyAllowed =
        gateState === 'enabled_ready' && decision.applyAllowed
      const appliedBusinessObject = integrationApplyAllowed
        ? appliedBusinessObjectForWrite(write, config, input.nowIso)
        : undefined
      const appliedReceiptRefs =
        appliedBusinessObject?.appliedReceiptRefs ?? []
      const closeoutReceiptRefs =
        appliedBusinessObject?.closeoutReceiptRefs ?? []

      return new OmniBusinessObjectDeliveryPlanEntry({
        appliedBusinessObject,
        appliedReceiptRefs,
        applyAllowed: integrationApplyAllowed,
        approvalRequired: decision.approvalRequired,
        blockerRefs:
          gateState === 'enabled_ready'
            ? decision.blockerRefs
            : [
              ...decision.blockerRefs,
              ...(gateState === 'inert_disabled'
                ? [
                  'blocker.business_object_delivery.integration_inert_disabled',
                ]
                : ['blocker.business_object_delivery.integration_enabled_blocked']),
            ].sort(),
        closeoutReceiptRefs,
        reasonRef:
          gateState === 'enabled_ready'
            ? decision.reasonRef
            : 'reason.business_object_delivery.held_inert',
        write: projectOmniBusinessObjectWrite(
          write,
          input.audience,
          input.nowIso,
        ),
      })
    },
  )

  return new OmniBusinessObjectDeliveryPlan({
    applyableCount: entries.filter(entry => entry.applyAllowed).length,
    audience: input.audience,
    blockerRefs: gateBlockerRefs(gateState, config),
    effectsApplied: entries.some(entry => entry.applyAllowed),
    entries,
    gateState,
    proposedCount: entries.length,
    workroomId: input.workroom.id,
  })
}

// ---------------------------------------------------------------------------
// Live-record integration: extract source-authority inputs from a live
// client-delivery workroom record and build the INERT delivery plan for it.
//
// This is the wiring that lets the live omni client-delivery workroom surface
// reach the source-authority + approval-gated write model at all. The source-
// authority bindings and proposed writes for a workroom live in the workroom
// record's projection-only `metadata.sourceAuthority` block (an existing D1
// field; no new migration, no new state). Extraction is decode-or-empty: any
// absent, malformed, or unsafe entry is dropped rather than thrown, so the
// integration can never crash a live workroom read and never fabricates a
// binding or write. The resulting plan applies only when the owner-gated
// delivery config reaches enabled_ready.
// ---------------------------------------------------------------------------

/**
 * The shape of the optional `sourceAuthority` block a live workroom record may
 * carry in its projection-only metadata. Both arrays are optional and each
 * entry is decoded independently so one bad entry never discards the rest.
 */
type WorkroomSourceAuthorityMetadata = Readonly<{
  bindings: ReadonlyArray<OmniSourceAuthorityBinding>
  config?: OmniBusinessObjectDeliveryConfig | undefined
  writes: ReadonlyArray<OmniBusinessObjectWriteRecord>
}>

const safeDeliveryConfigRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeDeliveryConfigRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|email|gho_|ghp_|invoice|lnbc|lntb|lno1|mnemonic|oauth|payment|payout|preimage|private|provider|raw|secret|sk-[a-z0-9]|token|wallet)/i

const safeConfigRef = (value: unknown): string | undefined => {
  if (typeof value !== 'string') {
    return undefined
  }

  const ref = value.trim()
  if (
    ref === '' ||
    !safeDeliveryConfigRefPattern.test(ref) ||
    unsafeDeliveryConfigRefPattern.test(ref)
  ) {
    return undefined
  }

  return ref
}

const decodeConfigSafely = (
  value: unknown,
): OmniBusinessObjectDeliveryConfig | undefined => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return undefined
  }

  const record = value as Readonly<Record<string, unknown>>
  const ownerSignOffRef = safeConfigRef(record['ownerSignOffRef'])
  const closeoutReceiptRef = safeConfigRef(record['closeoutReceiptRef'])

  return {
    closeoutReceiptRef,
    integrationEnabled: record['integrationEnabled'] === true,
    ownerSignOffRef,
  }
}

const decodeBindingsSafely = (
  value: unknown,
): ReadonlyArray<OmniSourceAuthorityBinding> => {
  if (!Array.isArray(value)) {
    return []
  }

  const decode = S.decodeUnknownSync(OmniSourceAuthorityBindingSchema)
  const out: Array<OmniSourceAuthorityBinding> = []

  for (const item of value) {
    try {
      out.push(decode(item))
    } catch {
      // Malformed or unsafe entry: skip it. The integration never fabricates a
      // binding, and one bad entry never discards the rest.
    }
  }

  return out
}

const decodeWritesSafely = (
  value: unknown,
): ReadonlyArray<OmniBusinessObjectWriteRecord> => {
  if (!Array.isArray(value)) {
    return []
  }

  const decode = S.decodeUnknownSync(OmniBusinessObjectWriteRecordSchema)
  const out: Array<OmniBusinessObjectWriteRecord> = []

  for (const item of value) {
    try {
      out.push(decode(item))
    } catch {
      // Malformed or unsafe entry: skip it.
    }
  }

  return out
}

/**
 * Pull the source-authority bindings + proposed writes a live workroom record
 * carries in `metadata.sourceAuthority`. Returns empty arrays when the block
 * is absent or unparseable; individual malformed entries are skipped.
 */
export const extractWorkroomSourceAuthorityInputs = (
  workroom: OmniWorkroomRecord,
): WorkroomSourceAuthorityMetadata => {
  const block = workroom.metadata?.['sourceAuthority']

  if (typeof block !== 'object' || block === null || Array.isArray(block)) {
    return { bindings: [], writes: [] }
  }

  const record = block as Readonly<Record<string, unknown>>

  return {
    bindings: decodeBindingsSafely(record['bindings']),
    config: decodeConfigSafely(record['config']),
    writes: decodeWritesSafely(record['writes']),
  }
}

/**
 * Build the source-authority delivery plan directly from a live workroom
 * record by extracting its `metadata.sourceAuthority` bindings/writes. This is
 * the entry point the live omni client-delivery workroom route surface uses to
 * project the source-authority model for a real workroom. The gate defaults to
 * `inert_disabled`; approved source-backed writes apply only when the owner-
 * gated config is present.
 */
export const buildOmniWorkroomSourceAuthorityDeliveryPlan = (
  input: Readonly<{
    audience: OmniProjectionAudience
    config?: OmniBusinessObjectDeliveryConfig | undefined
    nowIso: string
    workroom: OmniWorkroomRecord
  }>,
): OmniBusinessObjectDeliveryPlan => {
  const { bindings, config, writes } = extractWorkroomSourceAuthorityInputs(
    input.workroom,
  )

  return buildOmniBusinessObjectDeliveryPlan({
    audience: input.audience,
    bindings,
    config: input.config ?? config,
    nowIso: input.nowIso,
    workroom: input.workroom,
    writes,
  })
}
