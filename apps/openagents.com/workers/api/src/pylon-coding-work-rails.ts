/**
 * Coding-agent work classes on the Pluralis-campaign rails (openagents
 * issue #4861, Pluralis adaptation tracker #4862, master tracking
 * issue #4855).
 *
 * Contract-only glue, no hardware gate: the coding-agent work classes
 * (`claude_agent_task`, `codex_agent_task`) become the first consumers
 * of the three Pluralis rails so coding-agent Pylons get honest funnel
 * positions and priced availability:
 *
 * 1. RECEIPT TIERS (#4854) — verified coding closeouts from an `active`
 *    ladder state are compute-tier; BYOK readiness probes
 *    (`capability.pylon.local_claude_agent` /
 *    `capability.pylon.local_codex`) are presence-tier evidence; and
 *    bounded no-spend smokes (warmup-state) classify presence-tier by
 *    construction. The classification REUSES `classifyReceiptTier`; the
 *    catalog here only maps coding evidence kinds onto its work kinds.
 * 2. JOIN LADDER (#4848) — coding-agent Pylon evidence maps onto the
 *    existing lifecycle states: registered (in registry), qualified
 *    (readiness probe passed), warmup (bounded no-spend smoke
 *    verified), active (live accepted closeout). The projection helper
 *    lets the funnel ladder block render coding-capability rungs.
 * 3. ADMISSION GATES (#4852) — a seeded definition-only gate set for
 *    the coding host shape (Node/Bun runtime present, workspace-write
 *    sandbox supported, host-RAM headroom for an SDK session), reusing
 *    the existing `DeviceAdmissionGateDefinition` machinery and its
 *    `definitionsOnly` / `liveAdmissionClaim: false` posture.
 *
 * Claim discipline: everything exported here is a contract definition.
 * No live admission, payout, or promise-state claim is made by this
 * module, and the promise records it cites
 * (`autopilot.codex_probe_pylon_successor.v1`,
 * `pylon.local_claude_agent_bridge.v1`) are referenced read-only. This
 * module never reads a clock; it carries no timestamps at all.
 */
import {
  type AutopilotCodingAdapter,
  CLAUDE_AGENT_ADAPTER,
  CODEX_ADAPTER,
  adapterCapabilityRefs,
  adapterJobKinds,
} from './autopilot-work-adapter-selection'
import {
  type PylonJoinLifecycleLadderCount,
  type PylonJoinLifecycleState,
  pylonJoinLifecycleLadderRankByState,
  pylonJoinLifecycleStateLabelByState,
} from './pylon-join-lifecycle'
import {
  type DeviceAdmissionGateContract,
  type DeviceAdmissionGateDefinition,
  exportDeviceAdmissionGateContract,
} from './training-device-admission-gates'
import {
  type PresenceComputeWorkKind,
  type ReceiptTierClassification,
  classifyReceiptTier,
} from './training-presence-compute-receipts'

export const PylonCodingWorkRailsSchemaVersion =
  'openagents.pylon.coding_work_rails.v1'

export const PylonCodingLadderSchemaVersion =
  'openagents.pylon.coding_join_lifecycle_ladder.v1'

// The coding-agent assignment job kinds from the live Pylon assignment
// contract (`PylonApiAssignmentJobKind` in pylon-api.ts, the
// claude_agent_task lineage of #4755/#4756 and the Codex executor lane
// of CX issues #4788-#4792).
export const CodingWorkJobKinds = [
  'claude_agent_task',
  'codex_agent_task',
] as const
export type CodingWorkJobKind = (typeof CodingWorkJobKinds)[number]

export const CodingWorkClassRefs = [
  'work_class.coding.claude_agent_task',
  'work_class.coding.codex_agent_task',
] as const
export type CodingWorkClassRef = (typeof CodingWorkClassRefs)[number]

// Both adapters execute inside the same Pylon host process, so the
// host-shape admission gates are adapter-independent and bind to this
// shared session work class rather than being duplicated per adapter.
export const CodingAgentSessionWorkClassRef =
  'work_class.coding.local_coding_agent_session'

export type CodingWorkClassDefinition = Readonly<{
  adapter: AutopilotCodingAdapter
  capabilityRef: string
  jobKind: CodingWorkJobKind
  promiseRef: string
  readinessSchemaRef: string
  workClassRef: CodingWorkClassRef
}>

// The evidence kinds the coding rails understand, mapped onto the
// receipt-tier classifier's closed work-kind set. The mapping is the
// whole point: coding evidence reuses `classifyReceiptTier` instead of
// growing a parallel tier policy.
export const CodingWorkEvidenceKinds = [
  'accepted_closeout',
  'bounded_no_spend_smoke',
  'byok_readiness_probe',
] as const
export type CodingWorkEvidenceKind = (typeof CodingWorkEvidenceKinds)[number]

export const presenceComputeWorkKindByCodingEvidenceKind: Readonly<
  Record<CodingWorkEvidenceKind, PresenceComputeWorkKind>
> = Object.freeze({
  accepted_closeout: 'verified_closeout',
  bounded_no_spend_smoke: 'shadow_window_work',
  byok_readiness_probe: 'qualification_probe',
})

export type CodingWorkReceiptTierClassification = Readonly<{
  classification: ReceiptTierClassification
  evidenceKind: CodingWorkEvidenceKind
  workClassRef: CodingWorkClassRef
}>

// Bounded lists: a ladder projection call is a bounded batch, never an
// unbounded stream, and per-record evidence lists stay small enough to
// project publicly.
export const MAX_CODING_LADDER_EVIDENCE_RECORDS = 500
export const MAX_CODING_EVIDENCE_REFS_PER_KIND = 32

// One coding-capable Pylon's evidence, as the registry and receipt
// stores already know it. A record exists only for a registry-listed
// Pylon, so the floor rung is `registered`; every higher rung requires
// the refs that prove it.
export type CodingPylonLadderEvidence = Readonly<{
  acceptedCloseoutRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  pylonRef: string
  readinessProbeRefs: ReadonlyArray<string>
  smokeVerificationRefs: ReadonlyArray<string>
}>

export type CodingLadderEntry = Readonly<{
  codingCapabilityRefs: ReadonlyArray<string>
  ladderRank: number
  pylonRef: string
  state: PylonJoinLifecycleState
  stateLabel: string
}>

export type CodingCapabilityLadderRungs = Readonly<{
  byState: ReadonlyArray<PylonJoinLifecycleLadderCount>
  capabilityRef: string
  totalCount: number
}>

export type PylonCodingLadderProjection = Readonly<{
  byCapability: ReadonlyArray<CodingCapabilityLadderRungs>
  byState: ReadonlyArray<PylonJoinLifecycleLadderCount>
  caveatRefs: ReadonlyArray<string>
  contractDefinitionOnly: true
  entries: ReadonlyArray<CodingLadderEntry>
  nonCodingPylonCount: number
  schemaVersion: typeof PylonCodingLadderSchemaVersion
  sourceRefs: ReadonlyArray<string>
  totalCount: number
}>

export type PylonCodingWorkRailsContract = Readonly<{
  admissionGates: DeviceAdmissionGateContract
  contractDefinitionOnly: true
  evidenceKinds: ReadonlyArray<CodingWorkEvidenceKind>
  liveAdmissionClaim: false
  livePayoutClaim: false
  policyRefs: ReadonlyArray<string>
  promiseRefs: ReadonlyArray<string>
  schemaVersion: typeof PylonCodingWorkRailsSchemaVersion
  sessionWorkClassRef: typeof CodingAgentSessionWorkClassRef
  sourceRefs: ReadonlyArray<string>
  workClasses: ReadonlyArray<CodingWorkClassDefinition>
}>

export class CodingWorkRailsValidationError extends Error {
  readonly _tag = 'CodingWorkRailsValidationError'
}

export class CodingWorkRailsUnsafeError extends Error {
  readonly _tag = 'CodingWorkRailsUnsafeError'
}

// Same posture as the join-lifecycle and receipt-tier guards: pylon,
// capability, and evidence refs get a substring scan for private host,
// wallet, payment, payout, secret, or raw timestamp material before
// any record carrying them is considered projectable.
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|access[_-]?token|bearer|cookie|email|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mnemonic|oauth|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination)|preimage|private[_-]?key|secret|seed[_-]?phrase|serial[_-]?number|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const assertSafeRef = (label: string, ref: string): void => {
  if (
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  ) {
    throw new CodingWorkRailsUnsafeError(
      `${label} contains private host, wallet, payment, payout target, secret, or raw timestamp material.`,
    )
  }
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const deepFreeze = <T>(value: T): T => {
  if (typeof value === 'object' && value !== null && !Object.isFrozen(value)) {
    Object.freeze(value)
    Object.values(value).forEach(deepFreeze)
  }

  return value
}

// The typed coding work-class catalog. Every string here is a live
// contract constant, not an invented ref: capability refs and job
// kinds come straight from the adapter-selection policy (CX5 #4792),
// readiness schema refs from the Pylon bridge probes
// (apps/pylon/src/claude-agent.ts #4718, apps/pylon/src/codex-agent.ts
// #4788), and promise refs from the product-promise registry.
export const CODING_WORK_CLASS_CATALOG: ReadonlyArray<CodingWorkClassDefinition> =
  deepFreeze([
    {
      adapter: CLAUDE_AGENT_ADAPTER,
      capabilityRef: adapterCapabilityRefs[CLAUDE_AGENT_ADAPTER],
      jobKind: adapterJobKinds[CLAUDE_AGENT_ADAPTER],
      promiseRef: 'pylon.local_claude_agent_bridge.v1',
      readinessSchemaRef: 'openagents.pylon.claude_agent_readiness.v0.3',
      workClassRef: 'work_class.coding.claude_agent_task',
    },
    {
      adapter: CODEX_ADAPTER,
      capabilityRef: adapterCapabilityRefs[CODEX_ADAPTER],
      jobKind: adapterJobKinds[CODEX_ADAPTER],
      promiseRef: 'autopilot.codex_probe_pylon_successor.v1',
      readinessSchemaRef: 'openagents.pylon.codex_agent_readiness.v0.3',
      workClassRef: 'work_class.coding.codex_agent_task',
    },
  ])

export const codingWorkClassForJobKind = (
  jobKind: CodingWorkJobKind,
): CodingWorkClassDefinition =>
  CODING_WORK_CLASS_CATALOG.find(workClass => workClass.jobKind === jobKind)!

export const codingWorkClassForCapabilityRef = (
  capabilityRef: string,
): CodingWorkClassDefinition | undefined =>
  CODING_WORK_CLASS_CATALOG.find(
    workClass => workClass.capabilityRef === capabilityRef,
  )

/**
 * Classifies one unit of coding-agent work into a receipt tier by
 * mapping the coding evidence kind onto the receipt-tier classifier's
 * work kinds and delegating to `classifyReceiptTier` unchanged:
 *
 * - `byok_readiness_probe` -> `qualification_probe`: presence-tier
 *   evidence by taxonomy (the probe IS the evidence).
 * - `bounded_no_spend_smoke` -> `shadow_window_work`: presence-tier by
 *   construction, exactly like warmup-state shadow work.
 * - `accepted_closeout` -> `verified_closeout`: compute-tier ONLY from
 *   an `active` ladder state with verification outcome refs; anywhere
 *   earlier on the ladder it pays presence-tier as unmerged work.
 */
export const classifyCodingWorkReceiptTier = (
  input: Readonly<{
    evidenceKind: CodingWorkEvidenceKind
    joinLifecycleState: PylonJoinLifecycleState
    verificationOutcomeRefs: ReadonlyArray<string>
    workClassRef: CodingWorkClassRef
  }>,
): CodingWorkReceiptTierClassification => ({
  classification: classifyReceiptTier({
    joinLifecycleState: input.joinLifecycleState,
    verificationOutcomeRefs: input.verificationOutcomeRefs,
    workKind: presenceComputeWorkKindByCodingEvidenceKind[input.evidenceKind],
  }),
  evidenceKind: input.evidenceKind,
  workClassRef: input.workClassRef,
})

const assertAdmissibleLadderEvidence = (
  evidence: CodingPylonLadderEvidence,
): void => {
  assertSafeRef('Coding ladder pylon ref', evidence.pylonRef)

  const refLists: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = [
    ['Coding ladder accepted closeout ref', evidence.acceptedCloseoutRefs],
    ['Coding ladder capability ref', evidence.capabilityRefs],
    ['Coding ladder readiness probe ref', evidence.readinessProbeRefs],
    ['Coding ladder smoke verification ref', evidence.smokeVerificationRefs],
  ]

  for (const [label, refs] of refLists) {
    if (refs.length > MAX_CODING_EVIDENCE_REFS_PER_KIND) {
      throw new CodingWorkRailsValidationError(
        `Coding ladder evidence records carry at most ${MAX_CODING_EVIDENCE_REFS_PER_KIND} refs per evidence kind.`,
      )
    }

    for (const ref of refs) {
      assertSafeRef(label, ref)
    }
  }
}

/**
 * Maps one coding-capable Pylon's evidence onto the existing join
 * ladder, claiming only the rung the refs prove:
 *
 * - live accepted closeout refs -> `active`
 * - else verified bounded no-spend smoke refs -> `warmup`
 * - else passed readiness probe refs -> `qualified`
 * - else (registry presence alone) -> `registered`
 *
 * This is an evidence projection in the same spirit as
 * `joinLifecycleStateForFunnel`, not a transition: the reason-coded
 * transition machine in pylon-join-lifecycle is untouched and its
 * closed taxonomy needs no new codes.
 */
export const codingJoinLifecycleStateForEvidence = (
  evidence: CodingPylonLadderEvidence,
): PylonJoinLifecycleState => {
  if (evidence.acceptedCloseoutRefs.length > 0) {
    return 'active'
  }

  if (evidence.smokeVerificationRefs.length > 0) {
    return 'warmup'
  }

  if (evidence.readinessProbeRefs.length > 0) {
    return 'qualified'
  }

  return 'registered'
}

const codingCapabilityRefsForEvidence = (
  evidence: CodingPylonLadderEvidence,
): ReadonlyArray<string> =>
  uniqueRefs(
    evidence.capabilityRefs.filter(
      ref => codingWorkClassForCapabilityRef(ref) !== undefined,
    ),
  )

const countByState = (
  states: ReadonlyArray<PylonJoinLifecycleState>,
): ReadonlyArray<PylonJoinLifecycleLadderCount> =>
  [
    ...states.reduce((counts, state) => {
      counts.set(state, (counts.get(state) ?? 0) + 1)

      return counts
    }, new Map<string, number>()),
  ]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, count]) => ({ count, key }))

/**
 * The funnel-ladder-block projection for coding capability: how many
 * coding-capable Pylons sit at each rung, overall and per coding
 * capability ref. Pylons without a declared coding capability are
 * excluded from the rungs and surfaced honestly as
 * `nonCodingPylonCount` instead of being counted as rung zero.
 */
export const pylonCodingLadderProjection = (
  evidenceRecords: ReadonlyArray<CodingPylonLadderEvidence>,
): PylonCodingLadderProjection => {
  if (evidenceRecords.length > MAX_CODING_LADDER_EVIDENCE_RECORDS) {
    throw new CodingWorkRailsValidationError(
      `Coding ladder projection accepts at most ${MAX_CODING_LADDER_EVIDENCE_RECORDS} evidence records per call.`,
    )
  }

  for (const evidence of evidenceRecords) {
    assertAdmissibleLadderEvidence(evidence)
  }

  const codingRecords = evidenceRecords
    .map(evidence => ({
      codingCapabilityRefs: codingCapabilityRefsForEvidence(evidence),
      evidence,
    }))
    .filter(record => record.codingCapabilityRefs.length > 0)

  const entries = codingRecords
    .map(({ codingCapabilityRefs, evidence }): CodingLadderEntry => {
      const state = codingJoinLifecycleStateForEvidence(evidence)

      return {
        codingCapabilityRefs,
        ladderRank: pylonJoinLifecycleLadderRankByState[state],
        pylonRef: evidence.pylonRef,
        state,
        stateLabel: pylonJoinLifecycleStateLabelByState[state],
      }
    })
    .sort((left, right) => left.pylonRef.localeCompare(right.pylonRef))

  const byCapability = CODING_WORK_CLASS_CATALOG.map(
    (workClass): CodingCapabilityLadderRungs => {
      const capabilityEntries = entries.filter(entry =>
        entry.codingCapabilityRefs.includes(workClass.capabilityRef),
      )

      return {
        byState: countByState(capabilityEntries.map(entry => entry.state)),
        capabilityRef: workClass.capabilityRef,
        totalCount: capabilityEntries.length,
      }
    },
  )

  return deepFreeze({
    byCapability,
    byState: countByState(entries.map(entry => entry.state)),
    caveatRefs: [
      'caveat.public.pylon_coding_ladder.counts_and_refs_only_no_device_identifiers',
      'caveat.public.pylon_coding_ladder.ladder_position_is_contract_projection_not_live_device_claim',
    ],
    contractDefinitionOnly: true,
    entries,
    nonCodingPylonCount: evidenceRecords.length - codingRecords.length,
    schemaVersion: PylonCodingLadderSchemaVersion,
    sourceRefs: [
      'issue.github.openagents.4861',
      'route:/api/public/pylon-capacity-funnel',
    ],
    totalCount: entries.length,
  })
}

// Seeded coding-host gate set. These are DEFINITIONS demonstrating the
// reasoned-admission pattern for the coding session host shape, not
// live admission policy: no Pylon host has been measured against them,
// and no funnel row may cite them as a live admission claim until
// receipted host evidence exists. Presence kinds measure 1 (present /
// supported) or 0 (absent / unsupported) and gate `at_least 1`.
export const CODING_WORK_DEVICE_ADMISSION_GATE_SET: ReadonlyArray<DeviceAdmissionGateDefinition> =
  deepFreeze([
    {
      admittedReasonCode:
        'device_admission.public.admitted_node_or_bun_runtime_present',
      excludedReasonCode:
        'device_admission.public.excluded_node_or_bun_runtime_missing',
      gateRef: 'gate.device_admission.coding.node_or_bun_runtime_present.v1',
      rationale:
        'both coding adapters load their SDKs lazily inside the Pylon host process, so a host without a working Node or Bun runtime cannot start any coding SDK session; runtime presence is measured, not assumed.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'node_or_bun_runtime_present',
        threshold: 1,
        unit: 'present_one_or_zero',
      },
      sourceRefs: [
        'issue.github.openagents.4861',
        'issue.github.openagents.4862',
      ],
      workClassRef: CodingAgentSessionWorkClassRef,
    },
    {
      admittedReasonCode:
        'device_admission.public.admitted_workspace_write_sandbox_supported',
      excludedReasonCode:
        'device_admission.public.excluded_workspace_write_sandbox_unsupported',
      gateRef:
        'gate.device_admission.coding.workspace_write_sandbox_supported.v1',
      rationale:
        'coding assignments execute under a workspace-write sandbox pinned to the bounded working directory (the Codex executor lane resolves read-only anywhere as narrowing, never expanding); a host that cannot enforce workspace-write must be excluded with that stated reason rather than run unsandboxed.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'workspace_write_sandbox_supported',
        threshold: 1,
        unit: 'supported_one_or_zero',
      },
      sourceRefs: [
        'issue.github.openagents.4861',
        'issue.github.openagents.4862',
      ],
      workClassRef: CodingAgentSessionWorkClassRef,
    },
    {
      admittedReasonCode:
        'device_admission.public.admitted_sdk_session_host_ram_headroom_at_or_above_floor',
      excludedReasonCode:
        'device_admission.public.excluded_sdk_session_host_ram_headroom_below_floor',
      gateRef:
        'gate.device_admission.coding.sdk_session_host_ram_headroom_floor.v1',
      rationale:
        'one coding SDK session holds a model conversation, a workspace materialization, and a verification run in host memory at once; a host below the headroom floor stalls or OOM-kills mid-assignment, so the floor is measured per the same host-RAM probe kind the training gates use.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'host_ram_headroom_gb',
        threshold: 8,
        unit: 'gigabytes',
      },
      sourceRefs: [
        'issue.github.openagents.4861',
        'issue.github.openagents.4862',
      ],
      workClassRef: CodingAgentSessionWorkClassRef,
    },
  ])

/**
 * The versioned, frozen, JSON-able coding-work rails contract. Like
 * the receipt-tier and admission-gate contracts it composes,
 * `contractDefinitionOnly`, `liveAdmissionClaim: false`, and
 * `livePayoutClaim: false` make the non-claim explicit in the exported
 * payload itself: this classifies coding work classes onto the rails
 * and claims nothing about live admission, settlement, or promise
 * state.
 */
export const exportPylonCodingWorkRailsContract =
  (): PylonCodingWorkRailsContract =>
    deepFreeze({
      admissionGates: exportDeviceAdmissionGateContract(
        CODING_WORK_DEVICE_ADMISSION_GATE_SET,
      ),
      contractDefinitionOnly: true,
      evidenceKinds: CodingWorkEvidenceKinds,
      liveAdmissionClaim: false,
      livePayoutClaim: false,
      policyRefs: [
        'policy.public.coding_work_rails.accepted_closeouts_pay_compute_tier_only_from_active',
        'policy.public.coding_work_rails.byok_readiness_probes_are_presence_tier_evidence',
        'policy.public.coding_work_rails.host_gates_are_definitions_not_live_admission_claims',
        'policy.public.coding_work_rails.no_spend_smokes_classify_presence_tier_by_construction',
      ],
      promiseRefs: [
        'autopilot.codex_probe_pylon_successor.v1',
        'pylon.local_claude_agent_bridge.v1',
      ],
      schemaVersion: PylonCodingWorkRailsSchemaVersion,
      sessionWorkClassRef: CodingAgentSessionWorkClassRef,
      sourceRefs: [
        'docs/2026-06-12-coding-work-rails.md',
        'issue.github.openagents.4861',
        'issue.github.openagents.4862',
      ],
      workClasses: CODING_WORK_CLASS_CATALOG,
    })
