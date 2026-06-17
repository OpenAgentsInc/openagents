import { describe, expect, test } from 'vitest'

import {
  buildForgeCommandSystemInput,
  projectForgeCommandSystem,
} from './command-system'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T22:00:00.000Z',
  snapshotRef: 'command-snapshot.public.work_1',
  versionRef: 'command-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge command system projection', () => {
  test('projects public command catalog state as refs-only non-authoritative state', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      commands: [
        {
          capabilityRefs: ['capability.public.command.review'],
          commandDescriptorRefs: ['command-descriptor.public.review'],
          commandRef: 'command.public.review_changes',
          freshness: 'fresh',
          inputModeRefs: ['input-mode.public.keyboard'],
          kind: 'built_in',
          parserRefs: ['parser.public.typed_command'],
          plannerRefs: ['planner.public.command_route'],
          policyRefs: ['policy.public.command.review'],
          selectorRefs: ['semantic-selector.public.command.review'],
          state: 'available',
        },
        {
          commandDescriptorRefs: ['command-descriptor.public.fallback'],
          commandRef: 'command.public.old_alias',
          fallbackRefs: ['command.public.review_changes'],
          freshness: 'fresh',
          kind: 'slash_command',
          parserRefs: ['parser.public.slash'],
          plannerRefs: ['planner.public.command_route'],
          policyRefs: ['policy.public.command.review'],
          selectorRefs: ['semantic-selector.public.command.review'],
          state: 'unavailable',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      available: 1,
      blocked: 0,
      conflicted: 0,
      total: 2,
      unavailable: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      catalogWriteAuthority: false,
      commandExecutionAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      inputModeWriteAuthority: false,
      intentRoutingAuthority: false,
      keybindingWriteAuthority: false,
      parserExecutionAuthority: false,
      providerAuthority: false,
      publicClaimAuthority: false,
      retrievalRoutingAuthority: false,
      settingsWriteAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing command catalog state as empty', () => {
    const view = projectForgeCommandSystem({
      generatedAt: '2026-06-17T22:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.commands).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale command catalog evidence', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      commands: [
        {
          commandDescriptorRefs: ['command-descriptor.public.stale'],
          commandRef: 'command.public.stale',
          freshness: 'stale',
          kind: 'built_in',
          parserRefs: ['parser.public.command'],
          plannerRefs: ['planner.public.command'],
          policyRefs: ['policy.public.command'],
          selectorRefs: ['semantic-selector.public.command'],
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:stale-command-evidence:command.public.stale',
    )
  })

  test('blocks available commands without parser planner selector or policy refs', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      commands: [
        {
          commandDescriptorRefs: ['command-descriptor.public.no_route'],
          commandRef: 'command.public.no_route',
          freshness: 'fresh',
          kind: 'command_palette',
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:command-policy-missing:command.public.no_route',
    )
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:semantic-selector-missing:command.public.no_route',
    )
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:parser-ref-missing:command.public.no_route',
    )
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:planner-ref-missing:command.public.no_route',
    )
  })

  test('blocks conflicted commands without conflict refs', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      commands: [
        {
          commandDescriptorRefs: ['command-descriptor.public.conflicted'],
          commandRef: 'command.public.conflicted',
          freshness: 'fresh',
          kind: 'keybinding',
          state: 'conflicted',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:conflict-evidence-missing:command.public.conflicted',
    )
  })

  test('blocks unavailable commands without fallback refs', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      commands: [
        {
          commandDescriptorRefs: ['command-descriptor.public.unavailable'],
          commandRef: 'command.public.unavailable',
          freshness: 'fresh',
          kind: 'slash_command',
          state: 'unavailable',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:fallback-ref-missing:command.public.unavailable',
    )
  })

  test('omits unsafe private command material before projection', () => {
    const view = projectForgeCommandSystem({
      ...baseInput,
      blockerRefs: [
        'command-blocker.public.safe',
        'raw command /Users/christopher/command.log',
      ],
      commands: [
        {
          blockerRefs: ['entry-blocker.public.safe', 'raw shell sk-private'],
          capabilityRefs: ['capability.public.safe'],
          commandDescriptorRefs: ['command-descriptor.public.safe', 'command text rm -rf'],
          commandRef: 'command.public.safe',
          conflictRefs: ['conflict.public.safe'],
          fallbackRefs: ['fallback-command.public.safe'],
          freshness: 'fresh',
          inputModeRefs: ['input-mode.public.safe', 'private input token'],
          kind: 'built_in',
          parserRefs: ['parser.public.safe', 'raw prompt /Users/christopher/prompt.md'],
          plannerRefs: ['planner.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          selectorRefs: ['semantic-selector.public.safe'],
          state: 'available',
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.commands[0]?.commandDescriptorRefs).toEqual([
      'command-descriptor.public.safe',
    ])
    expect(view.commands[0]?.parserRefs).toEqual(['parser.public.safe'])
    expect(view.commands[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-command-system-blocker:work.public.work_1:unsafe-command-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('command text')
    expect(payload).not.toContain('private input')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      commandSystem: {
        commands: [
          {
            commandDescriptorRefs: ['command-descriptor.public.work_2'],
            commandRef: 'command.public.work_2',
            freshness: 'fresh',
            kind: 'built_in',
            parserRefs: ['parser.public.work_2'],
            plannerRefs: ['planner.public.work_2'],
            policyRefs: ['policy.public.work_2'],
            selectorRefs: ['semantic-selector.public.work_2'],
            state: 'available',
          },
        ],
        snapshotRef: 'command-snapshot.public.work_2',
        versionRef: 'command-version.public.v2',
      },
      generatedAt: '2026-06-17T22:01:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeCommandSystemInput(work)).toEqual({
      commands: [
        {
          commandDescriptorRefs: ['command-descriptor.public.work_2'],
          commandRef: 'command.public.work_2',
          freshness: 'fresh',
          kind: 'built_in',
          parserRefs: ['parser.public.work_2'],
          plannerRefs: ['planner.public.work_2'],
          policyRefs: ['policy.public.work_2'],
          selectorRefs: ['semantic-selector.public.work_2'],
          state: 'available',
        },
      ],
      generatedAt: '2026-06-17T22:01:00.000Z',
      snapshotRef: 'command-snapshot.public.work_2',
      versionRef: 'command-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
