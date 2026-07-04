import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { GymPage } from './-gym-page'

describe('Start Gym route', () => {
  test('server-renders the migrated Gym empty-state contracts', () => {
    const html = renderToStaticMarkup(<GymPage />)

    expect(html).toContain('data-route="gym"')
    expect(html).toContain('data-gym-page=""')
    expect(html).toContain('data-gym-no-spend-banner=""')
    expect(html).toContain('OpenAgents Gym')
    expect(html).toContain('Terminal-Bench 2.0')
    expect(html).toContain('data-gym-terminal-bench-panel=""')
    expect(html).toContain('data-gym-terminal-bench-empty=""')
    expect(html).toContain('No decision-grade benchmark reports published yet')
    expect(html).toContain('data-gym-run-progress-panel=""')
    expect(html).toContain('data-gym-run-progress-accessible-mirror=""')
    expect(html).toContain('data-gym-run-progress-empty=""')
    expect(html).toContain('No active Gym run')
  })

  test('keeps the public Gym route no-spend and free of fixture results', () => {
    const html = renderToStaticMarkup(<GymPage />)

    expect(html).toContain('Provider fan-out')
    expect(html).toContain('Program signature modules')
    expect(html).toContain('Locked to no spend')
    expect(html).toContain('fixture compile only')
    expect(html).not.toContain('data-gym-run=""')
    expect(html).not.toContain('data-gym-result')
    expect(html).not.toContain('Run fixture')
    expect(html).not.toContain('openagents.gym.fixture_report.v1')
    expect(html).not.toContain('private_openai_compat')
    expect(html).not.toContain('Bearer')
    expect(html).not.toContain('69.7')
    expect(html).not.toContain('41 of 89')
  })
})
