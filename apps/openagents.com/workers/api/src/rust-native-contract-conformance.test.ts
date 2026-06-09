import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
  OpenAgentsNativeContractRegistry,
  OpenAgentsNativeContractRegistryProjection,
  OpenAgentsNativeContractRegistryUnsafe,
  openAgentsNativeContractRegistryCoversConsumers,
  openAgentsNativeContractRegistryCoversRefKinds,
  openAgentsNativeContractRegistryProjectionHasPrivateMaterial,
  projectOpenAgentsNativeContractRegistry,
} from './native-contract-registry'
import {
  OPENAGENTS_OA_NODE_CONFORMANCE_FIXTURES,
  OpenAgentsOaNodeMachineProjection,
  OpenAgentsOaNodeMachineRecord,
  OpenAgentsOaNodeMachineUnsafe,
  openAgentsOaNodeMachineProjectionHasPrivateMaterial,
  openAgentsOaNodeManagedAvailable,
  projectOpenAgentsOaNodeMachine,
} from './oa-node-managed-machine'
import {
  OPENAGENTS_WORKROOMD_CONFORMANCE_FIXTURES,
  OpenAgentsWorkroomdSessionProjection,
  OpenAgentsWorkroomdSessionRecord,
  OpenAgentsWorkroomdSessionUnsafe,
  openAgentsWorkroomdSessionHasOnlyGrantRefs,
  openAgentsWorkroomdSessionPreservesAuditEvidence,
  openAgentsWorkroomdSessionProjectionHasPrivateMaterial,
  projectOpenAgentsWorkroomdSession,
} from './oa-workroomd-sidecar-contract'
import {
  OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES,
  OpenAgentsPylonSettlementBridgeProjection,
  OpenAgentsPylonSettlementBridgeRecord,
  OpenAgentsPylonSettlementBridgeUnsafe,
  openAgentsPylonSettlementBridgeHasNoSpendAuthority,
  openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial,
  projectOpenAgentsPylonSettlementBridge,
} from './pylon-settlement-bridge'
import {
  OPENAGENTS_PROBE_CONFORMANCE_FIXTURES,
  OpenAgentsProbeContractUnsafe,
  OpenAgentsProbeRunProjection,
  OpenAgentsProbeRunRecord,
  openAgentsProbeRunHasRequiredTerminalEvidence,
  openAgentsProbeRunIsTerminal,
  openAgentsProbeRunProjectionHasPrivateMaterial,
  projectOpenAgentsProbeRun,
} from './probe-coding-runtime-contract'
import {
  OPENAGENTS_PSIONIC_CONFORMANCE_FIXTURES,
  OpenAgentsPsionicEvidenceProjection,
  OpenAgentsPsionicEvidenceRecord,
  OpenAgentsPsionicEvidenceUnsafe,
  openAgentsPsionicEvidenceCanMutateRuntime,
  openAgentsPsionicEvidenceProjectionHasPrivateMaterial,
  projectOpenAgentsPsionicEvidence,
} from './psionic-evidence-contract'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T04:30:00.000Z'

const decodeRegistry = (): OpenAgentsNativeContractRegistry =>
  S.decodeUnknownSync(OpenAgentsNativeContractRegistry)(
    OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
  )

const decodeOaNodeFixture = (
  index: number,
): OpenAgentsOaNodeMachineRecord =>
  S.decodeUnknownSync(OpenAgentsOaNodeMachineRecord)(
    OPENAGENTS_OA_NODE_CONFORMANCE_FIXTURES[index],
  )

const decodeWorkroomdFixture = (
  index: number,
): OpenAgentsWorkroomdSessionRecord =>
  S.decodeUnknownSync(OpenAgentsWorkroomdSessionRecord)(
    OPENAGENTS_WORKROOMD_CONFORMANCE_FIXTURES[index],
  )

const decodeProbeFixture = (
  index: number,
): OpenAgentsProbeRunRecord =>
  S.decodeUnknownSync(OpenAgentsProbeRunRecord)(
    OPENAGENTS_PROBE_CONFORMANCE_FIXTURES[index],
  )

const decodePsionicFixture = (
  index: number,
): OpenAgentsPsionicEvidenceRecord =>
  S.decodeUnknownSync(OpenAgentsPsionicEvidenceRecord)(
    OPENAGENTS_PSIONIC_CONFORMANCE_FIXTURES[index],
  )

const decodePylonFixture = (
  index: number,
): OpenAgentsPylonSettlementBridgeRecord =>
  S.decodeUnknownSync(OpenAgentsPylonSettlementBridgeRecord)(
    OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES[index],
  )

const serializedHasNoRawTimestamp = (value: unknown): boolean =>
  !JSON.stringify(value).includes('2026-06-07T') &&
  !JSON.stringify(value).includes('2026-06-06T')

describe('Rust/native contract conformance fixtures', () => {
  test('validates shared registry fixture coverage and safe public projection', () => {
    const registry = decodeRegistry()
    const projection = projectOpenAgentsNativeContractRegistry(
      registry,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsNativeContractRegistryProjection)(
      projection,
    )).toEqual(projection)
    expect(openAgentsNativeContractRegistryCoversRefKinds(registry)).toBe(true)
    expect(openAgentsNativeContractRegistryCoversConsumers(registry)).toBe(true)
    expect(projection.entryCount).toBe(9)
    expect(projection.evidenceOnlyCount).toBe(7)
    expect(projection.authorityActionCount).toBe(2)
    expect(serializedHasNoRawTimestamp(projection)).toBe(true)
    expect(openAgentsNativeContractRegistryProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('validates oa-node fixtures and preserves managed-machine semantics', () => {
    const managedNode = decodeOaNodeFixture(0)
    const providerNode = decodeOaNodeFixture(1)
    const publicProjection = projectOpenAgentsOaNodeMachine(
      managedNode,
      'public',
      nowIso,
    )
    const operatorProjection = projectOpenAgentsOaNodeMachine(
      providerNode,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsOaNodeMachineProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(S.decodeUnknownSync(OpenAgentsOaNodeMachineProjection)(
      operatorProjection,
    )).toEqual(operatorProjection)
    expect(openAgentsOaNodeManagedAvailable(managedNode)).toBe(true)
    expect(operatorProjection.providerPayoutEligible).toBe(true)
    expect(publicProjection.operatorDiagnosticRefs).toEqual([])
    expect(serializedHasNoRawTimestamp(publicProjection)).toBe(true)
    expect(openAgentsOaNodeMachineProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
    expect(openAgentsSerializedValueContainsUnsafeFixture(operatorProjection))
      .toBe(false)
  })

  test('validates oa-workroomd fixtures and keeps grants operator-only', () => {
    const activeSession = decodeWorkroomdFixture(0)
    const archivedSession = decodeWorkroomdFixture(1)
    const publicProjection = projectOpenAgentsWorkroomdSession(
      activeSession,
      'public',
      nowIso,
    )
    const operatorProjection = projectOpenAgentsWorkroomdSession(
      activeSession,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsWorkroomdSessionProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(openAgentsWorkroomdSessionHasOnlyGrantRefs(activeSession)).toBe(true)
    expect(openAgentsWorkroomdSessionPreservesAuditEvidence(activeSession))
      .toBe(true)
    expect(openAgentsWorkroomdSessionPreservesAuditEvidence(archivedSession))
      .toBe(true)
    expect(publicProjection.grantRefs).toEqual([])
    expect(publicProjection.grantResolutionRefs).toEqual([])
    expect(operatorProjection.grantRefs).toEqual([
      'auth_grant.codex.account_1',
      'github_write_grant.otec.repo',
    ])
    expect(serializedHasNoRawTimestamp(publicProjection)).toBe(true)
    expect(openAgentsWorkroomdSessionProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
  })

  test('validates Probe, Psionic, and Pylon fixtures through schema and projection helpers', () => {
    const probeSuccess = decodeProbeFixture(0)
    const probeFailure = decodeProbeFixture(1)
    const psionicNeedsReview = decodePsionicFixture(0)
    const psionicScorecard = decodePsionicFixture(1)
    const pylonSettled = decodePylonFixture(0)
    const pylonBuyerPaymentOnly = decodePylonFixture(1)
    const probeProjection = projectOpenAgentsProbeRun(
      probeSuccess,
      'public',
      nowIso,
    )
    const psionicProjection = projectOpenAgentsPsionicEvidence(
      psionicNeedsReview,
      'public',
      nowIso,
    )
    const pylonProjection = projectOpenAgentsPylonSettlementBridge(
      pylonSettled,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsProbeRunProjection)(probeProjection))
      .toEqual(probeProjection)
    expect(S.decodeUnknownSync(OpenAgentsPsionicEvidenceProjection)(
      psionicProjection,
    )).toEqual(psionicProjection)
    expect(S.decodeUnknownSync(OpenAgentsPylonSettlementBridgeProjection)(
      pylonProjection,
    )).toEqual(pylonProjection)
    expect(openAgentsProbeRunIsTerminal(probeSuccess.status)).toBe(true)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(probeSuccess)).toBe(
      true,
    )
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(probeFailure)).toBe(
      true,
    )
    expect(openAgentsPsionicEvidenceCanMutateRuntime(psionicNeedsReview))
      .toBe(false)
    expect(projectOpenAgentsPsionicEvidence(
      psionicScorecard,
      'team',
      nowIso,
    ).status).toBe('completed')
    expect(openAgentsPylonSettlementBridgeHasNoSpendAuthority(
      pylonSettled.authority,
    )).toBe(true)
    expect(projectOpenAgentsPylonSettlementBridge(
      pylonBuyerPaymentOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      acceptedWorkClaimAllowed: false,
      buyerPaymentEvidencePresent: true,
      payoutEligibilityClaimAllowed: false,
      rewardIntentClaimAllowed: false,
      settlementClaimAllowed: false,
    })
    expect(serializedHasNoRawTimestamp(probeProjection)).toBe(true)
    expect(serializedHasNoRawTimestamp(psionicProjection)).toBe(true)
    expect(serializedHasNoRawTimestamp(pylonProjection)).toBe(true)
    expect(openAgentsProbeRunProjectionHasPrivateMaterial(probeProjection))
      .toBe(false)
    expect(openAgentsPsionicEvidenceProjectionHasPrivateMaterial(
      psionicProjection,
    )).toBe(false)
    expect(openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial(
      pylonProjection,
    )).toBe(false)
  })

  test('rejects or flags malformed native fixtures before Rust/native parity can pass', () => {
    const registryMissingConsumer = {
      ...decodeRegistry(),
      entries: decodeRegistry().entries.filter(entry =>
        !entry.consumerRefs.includes('treasury')
      ),
    }
    const probeSuccessMissingReceipt = {
      ...decodeProbeFixture(0),
      closeoutReceiptRefs: [],
    }
    const probeFailureMissingRetainedEvidence: OpenAgentsProbeRunRecord = {
      ...decodeProbeFixture(1),
      retainedFailureRefs: [],
      status: 'failed',
    }
    const pylonSettledMissingSettlement = {
      ...decodePylonFixture(0),
      settlementRefs: [],
    }

    expect(openAgentsNativeContractRegistryCoversConsumers(
      registryMissingConsumer,
    )).toBe(false)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(
      probeSuccessMissingReceipt,
    )).toBe(false)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(
      probeFailureMissingRetainedEvidence,
    )).toBe(false)
    expect(() =>
      projectOpenAgentsPylonSettlementBridge(
        pylonSettledMissingSettlement,
        'operator',
        nowIso,
      ),
    ).toThrow(OpenAgentsPylonSettlementBridgeUnsafe)
  })

  test('rejects secret-bearing refs, raw logs, timestamps, wallet/payment material, and private repos across native fixtures', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        projectOpenAgentsNativeContractRegistry(
          {
            ...decodeRegistry(),
            caveatRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsNativeContractRegistryUnsafe)
      expect(() =>
        projectOpenAgentsOaNodeMachine(
          {
            ...decodeOaNodeFixture(0),
            healthRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsOaNodeMachineUnsafe)
      expect(() =>
        projectOpenAgentsWorkroomdSession(
          {
            ...decodeWorkroomdFixture(0),
            grantRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsWorkroomdSessionUnsafe)
      expect(() =>
        projectOpenAgentsProbeRun(
          {
            ...decodeProbeFixture(0),
            failureRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsProbeContractUnsafe)
      expect(() =>
        projectOpenAgentsPsionicEvidence(
          {
            ...decodePsionicFixture(0),
            datasetRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPsionicEvidenceUnsafe)
      expect(() =>
        projectOpenAgentsPylonSettlementBridge(
          {
            ...decodePylonFixture(0),
            evidenceRefs: [fixture.value],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPylonSettlementBridgeUnsafe)
    }
  })
})
