import { describe, expect, test } from 'vitest'

import { LandingRoute } from '../../route'
import {
  ClickedCopyAgentInstructions,
  ClickedEnterKhala,
  ClickedEnterTassadar,
  ClickedExitKhala,
  CompletedCopyAgentInstructions,
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

  test('ClickedExitKhala returns to /landing (shared by both info pages)', () => {
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
