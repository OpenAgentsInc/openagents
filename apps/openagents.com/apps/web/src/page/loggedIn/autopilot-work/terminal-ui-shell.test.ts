import { describe, expect, test } from 'vitest'

import {
  buildForgeTerminalUiShellInput,
  projectForgeTerminalUiShell,
} from './terminal-ui-shell'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T21:00:00.000Z',
  snapshotRef: 'terminal-snapshot.public.work_1',
  versionRef: 'terminal-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge terminal UI shell projection', () => {
  test('projects public terminal surfaces as refs-only non-authoritative state', () => {
    const view = projectForgeTerminalUiShell({
      ...baseInput,
      surfaces: [
        {
          accessibilityRefs: ['accessibility.public.non_interactive'],
          commandDescriptorRefs: ['command-descriptor.public.review'],
          freshness: 'fresh',
          inputDescriptorRefs: ['input-descriptor.public.keys'],
          mode: 'interactive',
          nonInteractiveRefs: ['non-interactive.public.json'],
          paneRefs: ['terminal-pane.public.main'],
          parityRefs: ['parity.public.web_tui'],
          policyRefs: ['policy.public.terminal.interactive'],
          shellRefs: ['terminal-shell.public.pylon'],
          state: 'available',
          streamRefs: ['terminal-stream.public.work_1'],
          surfaceRef: 'terminal-surface.public.tui',
          transcriptSummaryRefs: ['terminal-transcript-summary.public.work_1'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      available: 1,
      blocked: 0,
      degraded: 0,
      interactive: 1,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      commandExecutionAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      inputInjectionAuthority: false,
      keybindingWriteAuthority: false,
      providerAuthority: false,
      ptyAuthority: false,
      publicClaimAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      terminalEmulatorAuthority: false,
      terminalProcessAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing terminal surfaces as empty', () => {
    const view = projectForgeTerminalUiShell({
      generatedAt: '2026-06-17T21:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.surfaces).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale terminal surface evidence', () => {
    const view = projectForgeTerminalUiShell({
      ...baseInput,
      surfaces: [
        {
          freshness: 'stale',
          mode: 'headless',
          shellRefs: ['terminal-shell.public.pylon'],
          state: 'degraded',
          surfaceRef: 'terminal-surface.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-terminal-ui-shell-blocker:work.public.work_1:stale-surface-evidence:terminal-surface.public.stale',
    )
  })

  test('blocks available surfaces without shell or stream evidence', () => {
    const view = projectForgeTerminalUiShell({
      ...baseInput,
      surfaces: [
        {
          freshness: 'fresh',
          mode: 'non_interactive',
          state: 'available',
          surfaceRef: 'terminal-surface.public.missing_evidence',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-terminal-ui-shell-blocker:work.public.work_1:missing-shell-evidence:terminal-surface.public.missing_evidence',
    )
    expect(view.blockerRefs).toContain(
      'forge-terminal-ui-shell-blocker:work.public.work_1:missing-stream-or-pane-evidence:terminal-surface.public.missing_evidence',
    )
  })

  test('blocks interactive available surfaces without policy refs', () => {
    const view = projectForgeTerminalUiShell({
      ...baseInput,
      surfaces: [
        {
          freshness: 'fresh',
          mode: 'interactive',
          paneRefs: ['terminal-pane.public.main'],
          shellRefs: ['terminal-shell.public.pylon'],
          state: 'available',
          surfaceRef: 'terminal-surface.public.no_policy',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-terminal-ui-shell-blocker:work.public.work_1:interactive-policy-missing:terminal-surface.public.no_policy',
    )
  })

  test('omits unsafe private terminal material before projection', () => {
    const view = projectForgeTerminalUiShell({
      ...baseInput,
      blockerRefs: [
        'terminal-blocker.public.safe',
        'raw terminal /Users/christopher/term.log',
      ],
      surfaces: [
        {
          accessibilityRefs: ['accessibility.public.safe'],
          blockerRefs: ['surface-blocker.public.safe', 'raw output sk-private'],
          commandDescriptorRefs: ['command-descriptor.public.safe', 'raw command rm -rf'],
          freshness: 'fresh',
          inputDescriptorRefs: ['input-descriptor.public.safe', 'private input token'],
          mode: 'interactive',
          nonInteractiveRefs: ['non-interactive.public.safe'],
          paneRefs: ['terminal-pane.public.safe'],
          parityRefs: ['parity.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          shellRefs: ['terminal-shell.public.safe'],
          state: 'available',
          streamRefs: ['terminal-stream.public.safe', 'terminal output /Users/christopher'],
          surfaceRef: 'terminal-surface.public.safe',
          transcriptSummaryRefs: [
            'terminal-transcript-summary.public.safe',
            'raw transcript ./private.log',
          ],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.surfaces[0]?.commandDescriptorRefs).toEqual([
      'command-descriptor.public.safe',
    ])
    expect(view.surfaces[0]?.streamRefs).toEqual(['terminal-stream.public.safe'])
    expect(view.surfaces[0]?.transcriptSummaryRefs).toEqual([
      'terminal-transcript-summary.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-terminal-ui-shell-blocker:work.public.work_1:unsafe-terminal-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw terminal')
    expect(payload).not.toContain('raw output')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('private input')
    expect(payload).not.toContain('terminal output')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T21:01:00.000Z',
      terminalUiShell: {
        snapshotRef: 'terminal-snapshot.public.work_2',
        surfaces: [
          {
            freshness: 'fresh',
            mode: 'headless',
            shellRefs: ['terminal-shell.public.headless'],
            state: 'available',
            streamRefs: ['terminal-stream.public.headless'],
            surfaceRef: 'terminal-surface.public.headless',
          },
        ],
        versionRef: 'terminal-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeTerminalUiShellInput(work)).toEqual({
      generatedAt: '2026-06-17T21:01:00.000Z',
      snapshotRef: 'terminal-snapshot.public.work_2',
      surfaces: [
        {
          freshness: 'fresh',
          mode: 'headless',
          shellRefs: ['terminal-shell.public.headless'],
          state: 'available',
          streamRefs: ['terminal-stream.public.headless'],
          surfaceRef: 'terminal-surface.public.headless',
        },
      ],
      versionRef: 'terminal-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
