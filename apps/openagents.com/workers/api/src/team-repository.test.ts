import { describe, expect, test } from 'vitest'

import { userTeamProjectAgentFromMetadata } from './team-repository'

describe('team repository helpers', () => {
  test('decodes complete project agent metadata', () => {
    expect(
      userTeamProjectAgentFromMetadata({
        agent: {
          backend: 'shc_vm',
          focus: 'customer support',
          id: 'agent_1',
          name: 'Support Autopilot',
          repository: 'OpenAgentsInc/autopilot-omega',
          runtime: 'codex',
          scope: 'team',
          status: 'active',
        },
      }),
    ).toEqual({
      backend: 'shc_vm',
      focus: 'customer support',
      id: 'agent_1',
      name: 'Support Autopilot',
      repository: 'OpenAgentsInc/autopilot-omega',
      runtime: 'codex',
      scope: 'team',
      status: 'active',
    })
  })

  test('decodes Adjutant project agent metadata for operator preflight', () => {
    expect(
      userTeamProjectAgentFromMetadata({
        agent: {
          backend: 'SHC',
          focus: 'Sites',
          id: 'agent_adjutant',
          name: 'Autopilot',
          repository: 'autopilot-omega',
          runtime: 'Autopilot',
          scope: 'project',
          status: 'active',
        },
        program: 'adjutant',
        surface: 'openagents-core-team',
      }),
    ).toEqual({
      backend: 'SHC',
      focus: 'Sites',
      id: 'agent_adjutant',
      name: 'Autopilot',
      repository: 'autopilot-omega',
      runtime: 'Autopilot',
      scope: 'project',
      status: 'active',
    })
  })

  test('rejects incomplete project agent metadata', () => {
    expect(
      userTeamProjectAgentFromMetadata({
        agent: {
          id: 'agent_1',
          name: 'Support Autopilot',
        },
      }),
    ).toBeUndefined()
  })
})
