import { describe, expect, test } from 'vitest'

import { LandingRoute } from '../../route'
import {
  ClickedRunGymFixture,
  ClickedCopyAgentInstructions,
  ClickedEnterKhala,
  ClickedEnterTassadar,
  ClickedExitKhala,
  CompletedCopyAgentInstructions,
  ToggledGymLane,
  UpdatedGymSamplesPerCell,
} from './message'
import { init } from './model'
import { TASSADAR_AGENT_INSTRUCTIONS, update } from './update'

const model = init(LandingRoute())

const commandNames = (commands: ReadonlyArray<{ readonly name: string }>) =>
  commands.map(command => command.name)

describe('logged-out nav + copy update', () => {
  test('ClickedEnterKhala navigates to /khala', () => {
    const [, commands] = update(model, ClickedEnterKhala())
    expect(commandNames(commands)).toEqual(['NavigateToKhala'])
  })

  test('ClickedEnterTassadar navigates to /tassadar', () => {
    const [, commands] = update(model, ClickedEnterTassadar())
    expect(commandNames(commands)).toEqual(['NavigateToTassadar'])
  })

  test('ClickedExitKhala returns home to / (shared by both info pages)', () => {
    const [, commands] = update(model, ClickedExitKhala())
    expect(commandNames(commands)).toEqual(['NavigateToLanding'])
  })

  test('ClickedCopyAgentInstructions issues a clipboard copy command', () => {
    const [, commands] = update(
      model,
      ClickedCopyAgentInstructions({ text: TASSADAR_AGENT_INSTRUCTIONS }),
    )
    expect(commandNames(commands)).toEqual(['CopyAgentInstructions'])
  })

  test('CompletedCopyAgentInstructions flips the "Copied" affirmation flag', () => {
    expect(model.copiedAgentInstructions).toBe(false)
    const [next, commands] = update(model, CompletedCopyAgentInstructions())
    expect(next.copiedAgentInstructions).toBe(true)
    expect(commands).toEqual([])
  })

  test('ClickedRunGymFixture materializes a no-spend report payload', () => {
    const [next, commands] = update(model, ClickedRunGymFixture())

    expect(commands).toEqual([])
    expect(next.gym.result).toMatchObject({
      viewerSchema: 'openagents.gym.fixture_report.v1',
      expectedCellCount: 90,
      metrics: { meanCostUsd: 0 },
    })
  })

  test('Gym lane toggles keep at least one provider lane selected', () => {
    const onlyLane = {
      ...model,
      gym: {
        ...model.gym,
        experiment: {
          ...model.gym.experiment,
          fanout: {
            ...model.gym.experiment.fanout,
            lanes: ['provider-baseline' as const],
          },
        },
      },
    }

    const [next] = update(
      onlyLane,
      ToggledGymLane({ lane: 'provider-baseline' }),
    )

    expect(next.gym.experiment.fanout.lanes).toEqual(['provider-baseline'])
  })

  test('Gym samples per cell clamps public fixture input', () => {
    const [next] = update(model, UpdatedGymSamplesPerCell({ value: '9000' }))

    expect(next.gym.experiment.samplesPerCell).toBe(25)
    expect(next.gym.result).toBeNull()
  })

  test('the copied agent instructions are grounded in AGENTS.md', () => {
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'POST https://openagents.com/api/agents/register',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain('npx @openagentsinc/pylon')
  })
})
