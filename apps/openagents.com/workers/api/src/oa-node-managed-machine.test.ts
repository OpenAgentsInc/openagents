import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsOaNodeMachineProjection,
  OpenAgentsOaNodeMachineRecord,
  OpenAgentsOaNodeMachineUnsafe,
  openAgentsOaNodeMachineProjectionHasPrivateMaterial,
  openAgentsOaNodeManagedAvailable,
  openAgentsOaNodeProviderPayoutEligible,
  projectOpenAgentsOaNodeMachine,
} from './oa-node-managed-machine'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T01:00:00.000Z'

const nodeRecord = (
  overrides: Partial<OpenAgentsOaNodeMachineRecord> = {},
): OpenAgentsOaNodeMachineRecord =>
  S.decodeUnknownSync(OpenAgentsOaNodeMachineRecord)({
    activeWorkroomRefs: ['workroom.site_public_otec'],
    artifactRefs: ['artifact.otec.preview_bundle'],
    availability: 'available',
    backendKind: 'shc_vm',
    capabilityRefs: ['capability.codex_exec', 'capability.site_build'],
    displayNameRef: 'oa_node.bertha_public',
    healthRefs: ['health.heartbeat_ok', 'health.disk_ok'],
    heartbeatRefs: ['heartbeat.oa_node.bertha.latest'],
    id: 'oa_node_machine.bertha',
    lastHeartbeatAtIso: '2026-06-07T00:58:00.000Z',
    machineKind: 'shc_vm',
    managedLiveness: 'healthy',
    maxWorkloadTrust: 'medium',
    nodeRef: 'oa_node.bertha',
    operatorCaveatRefs: ['operator.caveat.requires_manual_upgrade'],
    operatorDiagnosticRefs: ['operator.diagnostic.disk_pressure_ok'],
    placementEligibilityRefs: [
      'provider.eligibility.reviewed_private',
      'placement.eligible.site_build',
    ],
    policyRefs: ['policy.oa_node.no_raw_credentials'],
    providerPayoutEligibility: 'not_provider',
    providerPayoutEligibilityRefs: [],
    publicSummaryRef: 'summary.oa_node.available_for_site_build',
    quarantineState: 'none',
    receiptRefs: ['receipt.oa_node.heartbeat_seen'],
    supportedRuntimes: ['codex', 'opencode', 'shell'],
    trustTier: 'reviewed',
    updatedAtIso: '2026-06-07T00:59:00.000Z',
    workloadClasses: ['coding', 'site_build'],
    ...overrides,
  })

describe('OpenAgents oa-node managed-machine contract', () => {
  test('projects managed-machine liveness separately from provider payout eligibility', () => {
    const record = nodeRecord()
    const projection = projectOpenAgentsOaNodeMachine(
      record,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsOaNodeMachineProjection)(projection))
      .toEqual(projection)
    expect(openAgentsOaNodeManagedAvailable(record)).toBe(true)
    expect(openAgentsOaNodeProviderPayoutEligible(record)).toBe(false)
    expect(projection.managedAvailable).toBe(true)
    expect(projection.providerPayoutEligible).toBe(false)
    expect(projection.providerPayoutEligibility).toBe('not_provider')
    expect(projection.operatorCaveatRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.lastHeartbeatDisplay).toBe('2 minutes ago')
    expect(projection.updatedAtDisplay).toBe('1 minute ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsOaNodeMachineProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('blocks managed availability and payout eligibility for quarantined nodes', () => {
    const record = nodeRecord({
      availability: 'offline',
      providerPayoutEligibility: 'eligible_pending_settlement',
      providerPayoutEligibilityRefs: ['payout_eligibility.reviewed_provider'],
      quarantineState: 'quarantined',
      trustTier: 'quarantined',
    })
    const projection = projectOpenAgentsOaNodeMachine(
      record,
      'operator',
      nowIso,
    )

    expect(openAgentsOaNodeManagedAvailable(record)).toBe(false)
    expect(openAgentsOaNodeProviderPayoutEligible(record)).toBe(false)
    expect(projection.managedAvailable).toBe(false)
    expect(projection.providerPayoutEligible).toBe(false)
    expect(projection.quarantineState).toBe('quarantined')
  })

  test('shows safe provider payout eligibility refs only to operator/private audiences', () => {
    const record = nodeRecord({
      machineKind: 'pylon_candidate',
      providerPayoutEligibility: 'eligible_pending_settlement',
      providerPayoutEligibilityRefs: [
        'payout_eligibility.pylon_provider.reviewed',
      ],
      trustTier: 'verified',
      workloadClasses: ['pylon_provider', 'site_build'],
    })
    const publicProjection = projectOpenAgentsOaNodeMachine(
      record,
      'public',
      nowIso,
    )
    const operatorProjection = projectOpenAgentsOaNodeMachine(
      record,
      'operator',
      nowIso,
    )

    expect(publicProjection.providerPayoutEligible).toBe(true)
    expect(publicProjection.providerPayoutEligibilityRefs).toEqual([])
    expect(operatorProjection.providerPayoutEligibilityRefs).toEqual([
      'payout_eligibility.pylon_provider.reviewed',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(operatorProjection))
      .toBe(false)
  })

  test('rejects unsafe host, path, auth, wallet, payment, and timestamp refs', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'hostname', value: 'hostname.bertha.local' },
      { label: 'local path', value: '/Users/chris/work/private' },
      { label: 'private network', value: '192.168.1.20' },
    ]) {
      expect(() =>
        projectOpenAgentsOaNodeMachine(
          nodeRecord({ healthRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsOaNodeMachineUnsafe)
    }
  })
})
