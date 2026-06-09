import { describe, expect, test } from 'vitest'

import {
  ARTANIS_AUTOPILOT_OPERATOR_ENDPOINTS,
  ARTANIS_OPERATOR_APPROVAL_ACTIONS,
  ARTANIS_OPERATOR_GOAL_ACTIONS,
  ArtanisOperatorGoalCommandRecord,
  ArtanisOperatorSteeringUnsafe,
  ArtanisOperatorSteeringWorkspaceRecord,
  artanisOperatorGoalLifecycleReady,
  artanisOperatorProjectionHasPrivateMaterial,
  exampleArtanisOperatorSteeringWorkspace,
  projectArtanisOperatorSteeringWorkspace,
} from './artanis-operator-steering'

const nowIso = '2026-06-06T17:30:00.000Z'

describe('Artanis operator steering contract', () => {
  test('projects the operator console with lifecycle endpoints and approval decisions', () => {
    const projection = projectArtanisOperatorSteeringWorkspace(
      exampleArtanisOperatorSteeringWorkspace,
      'operator',
      nowIso,
    )

    expect(artanisOperatorGoalLifecycleReady(
      exampleArtanisOperatorSteeringWorkspace,
    )).toBe(true)
    expect(projection.agentId).toBe('agent_artanis')
    expect(projection.supportedGoalActions).toEqual(ARTANIS_OPERATOR_GOAL_ACTIONS)
    expect(projection.supportedApprovalActions).toEqual([
      ...ARTANIS_OPERATOR_APPROVAL_ACTIONS,
    ])
    expect(projection.operatorEndpoints).toEqual(
      ARTANIS_AUTOPILOT_OPERATOR_ENDPOINTS,
    )
    expect(projection.goalCommands.map(command => command.kind)).toEqual([
      'create_goal',
      'reprioritize_goal',
      'pause_goal',
      'resume_goal',
      'cancel_goal',
    ])
    expect(projection.privateEvidencePackRefs).toEqual([
      'evidence.private.artanis.operator_pack',
    ])
    expect(projection.rawWorkroomStateRefs).toEqual([
      'workroom.private.artanis.loop_state',
    ])
    expect(projection.approvalDecisions.map(decision => decision.state)).toEqual([
      'approved',
      'rejected',
    ])
  })

  test('keeps public Artanis and Forum projections downstream of operator evidence', () => {
    const artanisProjection = projectArtanisOperatorSteeringWorkspace(
      exampleArtanisOperatorSteeringWorkspace,
      'public_artanis',
      nowIso,
    )
    const forumProjection = projectArtanisOperatorSteeringWorkspace(
      exampleArtanisOperatorSteeringWorkspace,
      'public_forum',
      nowIso,
    )
    const serializedArtanis = JSON.stringify(artanisProjection)
    const serializedForum = JSON.stringify(forumProjection)

    expect(artanisProjection.operatorEndpoints).toEqual([])
    expect(forumProjection.operatorEndpoints).toEqual([])
    expect(artanisProjection.supportedGoalActions).toEqual([])
    expect(forumProjection.supportedApprovalActions).toEqual([])
    expect(artanisProjection.privateEvidencePackRefs).toEqual([])
    expect(artanisProjection.rawWorkroomStateRefs).toEqual([])
    expect(forumProjection.privateEvidencePackRefs).toEqual([])
    expect(forumProjection.rawWorkroomStateRefs).toEqual([])
    expect(serializedArtanis).not.toContain('evidence.private')
    expect(serializedArtanis).not.toContain('workroom.private')
    expect(serializedArtanis).not.toContain('receipt.operator')
    expect(serializedArtanis).not.toContain('steering.private')
    expect(serializedForum).not.toContain('evidence.private')
    expect(serializedForum).not.toContain('workroom.private')
    expect(serializedForum).not.toContain('receipt.operator')
    expect(serializedForum).not.toContain('steering.private')
    expect(artanisOperatorProjectionHasPrivateMaterial(artanisProjection)).toBe(
      false,
    )
    expect(artanisOperatorProjectionHasPrivateMaterial(forumProjection)).toBe(
      false,
    )
  })

  test('projects public state only from accepted or completed commands', () => {
    const baseCreateCommand =
      exampleArtanisOperatorSteeringWorkspace.goalCommands.find(
        command => command.kind === 'create_goal',
      )

    if (baseCreateCommand === undefined) {
      throw new Error('example workspace must include create_goal command')
    }

    const blockedCommand = new ArtanisOperatorGoalCommandRecord({
      ...baseCreateCommand,
      blockerRefs: ['blocker.public.operator_waiting'],
      commandRef: 'command.public.artanis.blocked_create',
      idempotencyKey: 'artanis-operator:blocked-create:v1',
      publicProjectionRefs: ['projection.public.artanis.should_not_show'],
      state: 'blocked',
    })
    const workspace = new ArtanisOperatorSteeringWorkspaceRecord({
      ...exampleArtanisOperatorSteeringWorkspace,
      goalCommands: [
        blockedCommand,
        ...exampleArtanisOperatorSteeringWorkspace.goalCommands.slice(1),
      ],
    })
    const projection = projectArtanisOperatorSteeringWorkspace(
      workspace,
      'public_artanis',
      nowIso,
    )

    expect(projection.goalCommands[0]?.state).toBe('blocked')
    expect(projection.goalCommands[0]?.publicProjectionRefs).toEqual([])
  })

  test('rejects non-Artanis targets, incomplete lifecycle support, and unsafe refs', () => {
    const nonArtanis = new ArtanisOperatorSteeringWorkspaceRecord({
      ...exampleArtanisOperatorSteeringWorkspace,
      agentId: 'agent_adjutant',
    })
    const missingLifecycle = new ArtanisOperatorSteeringWorkspaceRecord({
      ...exampleArtanisOperatorSteeringWorkspace,
      goalCommands:
        exampleArtanisOperatorSteeringWorkspace.goalCommands.filter(
          command => command.kind !== 'reprioritize_goal',
        ),
    })
    const unsafe = new ArtanisOperatorSteeringWorkspaceRecord({
      ...exampleArtanisOperatorSteeringWorkspace,
      privateEvidencePackRefs: ['evidence.private.artanis.token-secret'],
    })

    expect(() =>
      projectArtanisOperatorSteeringWorkspace(nonArtanis, 'operator', nowIso),
    ).toThrow(ArtanisOperatorSteeringUnsafe)
    expect(() =>
      projectArtanisOperatorSteeringWorkspace(
        missingLifecycle,
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisOperatorSteeringUnsafe)
    expect(() =>
      projectArtanisOperatorSteeringWorkspace(unsafe, 'operator', nowIso),
    ).toThrow(ArtanisOperatorSteeringUnsafe)
  })
})
