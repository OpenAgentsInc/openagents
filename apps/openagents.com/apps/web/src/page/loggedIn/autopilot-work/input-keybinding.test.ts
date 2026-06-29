import { describe, expect, test } from 'vitest'

import {
  buildForgeInputKeybindingInput,
  projectForgeInputKeybinding,
} from './input-keybinding'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T21:30:00.000Z',
  snapshotRef: 'input-keybinding-snapshot.public.work_1',
  versionRef: 'input-keybinding-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge input/keybinding projection', () => {
  test('projects public input modes as refs-only non-authoritative state', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      entries: [
        {
          accessibilityRefs: ['accessibility.public.keyboard'],
          bindingMapRefs: ['binding-map.public.default'],
          commandDescriptorRefs: ['command-descriptor.public.review'],
          conflictRefs: [],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.keyboard',
          keymapRefs: ['keymap.public.default'],
          mode: 'keyboard',
          nonInteractiveFallbackRefs: ['non-interactive.public.json'],
          platformRefs: ['platform.public.mac'],
          policyRefs: ['policy.public.input.keyboard'],
          state: 'available',
        },
        {
          commandDescriptorRefs: ['command-descriptor.public.headless'],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.headless_json',
          mode: 'headless_json',
          policyRefs: ['policy.public.input.headless'],
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      available: 2,
      blocked: 0,
      conflicts: 0,
      interactive: 1,
      total: 2,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      commandExecutionAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      inputCaptureAuthority: false,
      inputInjectionAuthority: false,
      inputModeWriteAuthority: false,
      keybindingExecutionAuthority: false,
      keybindingWriteAuthority: false,
      providerAuthority: false,
      publicClaimAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      terminalProcessAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing input/keybinding state as empty', () => {
    const view = projectForgeInputKeybinding({
      generatedAt: '2026-06-17T21:30:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale input/keybinding evidence', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      entries: [
        {
          commandDescriptorRefs: ['command-descriptor.public.stale'],
          freshness: 'stale',
          inputModeRef: 'input-mode.public.stale',
          mode: 'keyboard',
          nonInteractiveFallbackRefs: ['non-interactive.public.json'],
          policyRefs: ['policy.public.input'],
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:stale-input-evidence:input-mode.public.stale',
    )
  })

  test('blocks available input without command descriptors or policy refs', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.no_command',
          mode: 'headless_json',
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:input-policy-missing:input-mode.public.no_command',
    )
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:command-descriptor-missing:input-mode.public.no_command',
    )
  })

  test('blocks interactive input without non-interactive fallback refs', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      entries: [
        {
          commandDescriptorRefs: ['command-descriptor.public.palette'],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.palette',
          mode: 'command_palette',
          policyRefs: ['policy.public.input.palette'],
          state: 'available',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:non-interactive-fallback-missing:input-mode.public.palette',
    )
  })

  test('blocks degraded keymaps without conflict refs', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      entries: [
        {
          commandDescriptorRefs: ['command-descriptor.public.degraded'],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.degraded',
          keymapRefs: ['keymap.public.degraded'],
          mode: 'keyboard',
          nonInteractiveFallbackRefs: ['non-interactive.public.json'],
          policyRefs: ['policy.public.input.keyboard'],
          state: 'degraded',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:conflict-evidence-missing:input-mode.public.degraded',
    )
  })

  test('omits unsafe private input/keybinding material before projection', () => {
    const view = projectForgeInputKeybinding({
      ...baseInput,
      blockerRefs: [
        'input-blocker.public.safe',
        'raw input /Users/christopher/input.log',
      ],
      entries: [
        {
          accessibilityRefs: ['accessibility.public.safe'],
          bindingMapRefs: ['binding-map.public.safe', 'key log sk-private'],
          blockerRefs: ['entry-blocker.public.safe', 'raw key /Users/christopher/key.log'],
          commandDescriptorRefs: ['command-descriptor.public.safe', 'raw command rm -rf'],
          conflictRefs: ['conflict.public.safe'],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.safe',
          keymapRefs: ['keymap.public.safe', 'private key token'],
          mode: 'keyboard',
          nonInteractiveFallbackRefs: ['non-interactive.public.safe'],
          platformRefs: ['platform.public.safe', 'private input token'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          state: 'available',
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.bindingMapRefs).toEqual(['binding-map.public.safe'])
    expect(view.entries[0]?.commandDescriptorRefs).toEqual([
      'command-descriptor.public.safe',
    ])
    expect(view.entries[0]?.keymapRefs).toEqual(['keymap.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-input-keybinding-blocker:work.public.work_1:unsafe-input-keybinding-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw input')
    expect(payload).not.toContain('key log')
    expect(payload).not.toContain('raw key')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('private key')
    expect(payload).not.toContain('private input')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T21:31:00.000Z',
      inputKeybinding: {
        entries: [
          {
            commandDescriptorRefs: ['command-descriptor.public.work_2'],
            freshness: 'fresh',
            inputModeRef: 'input-mode.public.work_2',
            mode: 'headless_json',
            policyRefs: ['policy.public.work_2'],
            state: 'available',
          },
        ],
        snapshotRef: 'input-keybinding-snapshot.public.work_2',
        versionRef: 'input-keybinding-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeInputKeybindingInput(work)).toEqual({
      entries: [
        {
          commandDescriptorRefs: ['command-descriptor.public.work_2'],
          freshness: 'fresh',
          inputModeRef: 'input-mode.public.work_2',
          mode: 'headless_json',
          policyRefs: ['policy.public.work_2'],
          state: 'available',
        },
      ],
      generatedAt: '2026-06-17T21:31:00.000Z',
      snapshotRef: 'input-keybinding-snapshot.public.work_2',
      versionRef: 'input-keybinding-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
