import { describe, expect, test } from 'vitest'

import { selectAutopilotPlacement } from './autopilot-work-placement-selector'
import type { PylonApiRegistrationRecord } from './pylon-api'

const registration = (
  override: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: [
    'capability.pylon.assignment_ready',
    'capability.pylon.local_codex',
  ],
  clientProtocolVersion: '0.2.5',
  clientVersion: '0.2.5',
  createdAt: '2026-06-09T17:25:00.000Z',
  displayName: 'Requester Pylon',
  id: 'pylon_registration_1',
  latestCapacityRefs: ['capacity.pylon.assignment_ready'],
  latestHeartbeatAt: '2026-06-09T17:29:30.000Z',
  latestHeartbeatStatus: 'ready',
  latestHealthRefs: ['health.pylon.ready'],
  latestLoadRefs: ['load.pylon.available'],
  latestResourceMode: 'balanced',
  ownerAgentCredentialId: 'agent_credential_autopilot_work_test',
  ownerAgentTokenPrefix: 'oa_agent',
  ownerAgentUserId: 'agent_user_autopilot_work',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.local.docs_agent',
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: '2026-06-09T17:29:30.000Z',
  walletReady: true,
  walletRef: 'wallet_ref.pylon.local.docs_agent',
  ...override,
})

const placementPolicy = {
  allowedRunnerKinds: ['requester_pylon', 'openagents_shc', 'cloud_sandbox'],
  disallowedRunnerKinds: [],
  localOnlyAllowed: false,
  preferredRunnerKinds: ['requester_pylon', 'openagents_shc'],
  privacyTier: 'public_beta',
  publicTraceAllowed: true,
  requiresSecretBroker: false,
} as const

describe('Autopilot work placement selector', () => {
  test('selects an online compatible owner-linked requester Pylon before fallback', () => {
    const decision = selectAutopilotPlacement({
      nowIso: '2026-06-09T17:30:00.000Z',
      ownerAgentUserId: 'agent_user_autopilot_work',
      placementPolicy,
      pylonRegistrations: [
        registration({ pylonRef: 'pylon.other', ownerAgentUserId: 'other' }),
        registration(),
      ],
    })

    expect(decision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      reasonRefs: [
        'placement.selected.requester_pylon',
        'placement.pylon.preferred_before_fallback',
      ],
      selectedPylonRef: 'pylon.local.docs_agent',
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(decision.pylonCandidates).toEqual([
      expect.objectContaining({
        ownerLinked: false,
        selected: false,
      }),
      expect.objectContaining({
        assignmentReady: true,
        heartbeatFresh: true,
        localExecutionReady: true,
        ownerLinked: true,
        selected: true,
        versionCompatible: true,
        walletReady: true,
      }),
    ])
  })

  test('falls back when requester Pylon is stale or incompatible', () => {
    const decision = selectAutopilotPlacement({
      nowIso: '2026-06-09T17:30:00.000Z',
      ownerAgentUserId: 'agent_user_autopilot_work',
      placementPolicy,
      pylonRegistrations: [
        registration({
          capabilityRefs: [],
          clientVersion: '0.2.4',
          latestHeartbeatAt: '2026-06-09T17:00:00.000Z',
          latestHeartbeatStatus: 'offline',
          walletReady: false,
        }),
      ],
    })

    expect(decision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      reasonRefs: [
        'placement.selected.fallback',
        'placement.fallback.openagents_shc',
      ],
      selectedPylonRef: null,
      selectedRunnerKind: 'openagents_shc',
      source: 'fallback',
    })
    expect(decision.pylonCandidates[0]).toMatchObject({
      assignmentReady: false,
      heartbeatFresh: false,
      selected: false,
      versionCompatible: false,
      walletReady: false,
    })
  })

  test('distinguishes local execution capability from network capacity', () => {
    const decision = selectAutopilotPlacement({
      nowIso: '2026-06-09T17:30:00.000Z',
      ownerAgentUserId: 'agent_user_autopilot_work',
      placementPolicy,
      pylonRegistrations: [
        registration({
          capabilityRefs: ['capability.pylon.assignment_ready'],
        }),
      ],
    })

    expect(decision).toMatchObject({
      selectedPylonRef: null,
      selectedRunnerKind: 'openagents_shc',
      source: 'fallback',
    })
    expect(decision.pylonCandidates[0]).toMatchObject({
      assignmentReady: true,
      localExecutionReady: false,
      reasonRefs: expect.arrayContaining([
        'placement.pylon.local_execution_missing',
      ]),
      selected: false,
    })
  })
})
