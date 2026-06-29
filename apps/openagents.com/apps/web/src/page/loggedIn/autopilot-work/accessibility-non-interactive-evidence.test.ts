import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeAccessibilityNonInteractiveInput,
  projectForgeAccessibilityNonInteractiveEvidence,
} from './accessibility-non-interactive-evidence'

const baseInput = {
  generatedAt: '2026-06-18T12:00:00.000Z',
  snapshotRef: 'accessibility-non-interactive-snapshot.public.work_1',
  versionRef: 'accessibility-non-interactive-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyCiMode = {
  ciPolicyRefs: ['ci-policy.public.no_mutation_default'],
  deployCaveatRefs: ['deploy-caveat.public.disabled'],
  exitCodeRefs: ['exit-code.public.stable'],
  freshness: 'fresh' as const,
  highContrastRefs: ['high-contrast.public.available'],
  keyboardNavigationRefs: ['keyboard-navigation.public.available'],
  mode: 'ci' as const,
  modeRef: 'interaction-mode.public.ci',
  noColorRefs: ['no-color.public.status_labels'],
  notificationAvailabilityRefs: ['notification.public.structured'],
  promptAvailabilityRefs: ['prompt-availability.public.none_required'],
  providerMutationCaveatRefs: ['provider-mutation.public.disabled'],
  pushCaveatRefs: ['push-caveat.public.disabled'],
  reducedMotionRefs: ['reduced-motion.public.no_spinners'],
  remoteBridgeAvailabilityRefs: ['remote-bridge.public.unavailable'],
  schemaRefs: ['schema.public.structured_output_v1'],
  spendCaveatRefs: ['spend-caveat.public.disabled'],
  status: 'ready' as const,
  statusLabelRefs: ['status-label.public.ready'],
  structuredOutputRefs: ['structured-output.public.json'],
  terminalCapabilityRefs: ['terminal-capability.public.ci'],
}

const readyScreenReaderMode = {
  ...readyCiMode,
  ciPolicyRefs: [],
  deployCaveatRefs: [],
  keyboardNavigationRefs: ['keyboard-navigation.public.full'],
  mode: 'screen_reader' as const,
  modeRef: 'interaction-mode.public.screen_reader',
  providerMutationCaveatRefs: [],
  pushCaveatRefs: [],
  screenReaderStatusRefs: ['screen-reader-status.public.labels'],
  spendCaveatRefs: [],
  terminalCapabilityRefs: ['terminal-capability.public.screen_reader'],
}

describe('Forge accessibility and non-interactive evidence projection', () => {
  test('projects accessibility and non-interactive evidence as refs-only non-authoritative state', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [readyCiMode, readyScreenReaderMode],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      ci: 1,
      headless: 0,
      nonInteractive: 1,
      ready: 2,
      screenReader: 1,
      stale: 0,
      total: 2,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      approvalGrantAuthority: false,
      approvalPolicyMutationAuthority: false,
      deployAuthority: false,
      exitCodeMutationAuthority: false,
      headlessCommandExecutionAuthority: false,
      liveSpendAuthority: false,
      preferenceWriteAuthority: false,
      promptAnswerAuthority: false,
      providerAccountMutationAuthority: false,
      pushAuthority: false,
      remoteBridgeStartAuthority: false,
      settlementAuthority: false,
      structuredOutputEmitAuthority: false,
      terminalCapabilityMutationAuthority: false,
      themeInstallAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing accessibility evidence as empty', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      generatedAt: '2026-06-18T12:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks ready modes without structured output schema status label no-color or exit-code evidence', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCiMode,
          exitCodeRefs: [],
          noColorRefs: [],
          schemaRefs: [],
          statusLabelRefs: [],
          structuredOutputRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:ready-mode-accessibility-contract-missing:interaction-mode.public.ci',
    )
  })

  test('blocks non-interactive prompt-required modes without resolver or typed prompt blocker refs', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCiMode,
          approvalResolverRefs: [],
          promptAvailabilityRefs: ['prompt-required.public.approval'],
          typedPromptBlockerRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:non-interactive-prompt-blocker-missing:interaction-mode.public.ci',
    )
  })

  test('allows non-interactive prompt-required modes with typed prompt blocker refs', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCiMode,
          approvalResolverRefs: [],
          promptAvailabilityRefs: ['prompt-required.public.approval'],
          typedPromptBlockerRefs: ['typed-prompt-blocker.public.approval_required'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks CI modes missing safety caveat refs', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCiMode,
          deployCaveatRefs: [],
          providerMutationCaveatRefs: [],
          pushCaveatRefs: [],
          spendCaveatRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:ci-safety-caveat-missing:interaction-mode.public.ci',
    )
  })

  test('blocks screen-reader modes missing screen-reader status or keyboard navigation refs', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyScreenReaderMode,
          keyboardNavigationRefs: [],
          screenReaderStatusRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:screen-reader-evidence-missing:interaction-mode.public.screen_reader',
    )
  })

  test('blocks stale accessibility evidence', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCiMode,
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:stale-accessibility-non-interactive-evidence:interaction-mode.public.ci',
    )
  })

  test('blocks populated accessibility entries without snapshot refs', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      entries: [readyCiMode],
      generatedAt: '2026-06-18T12:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.no_snapshot:missing-accessibility-non-interactive-snapshot-ref',
    )
  })

  test('omits unsafe private accessibility material before projection', () => {
    const view = projectForgeAccessibilityNonInteractiveEvidence({
      ...baseInput,
      blockerRefs: [
        'accessibility-blocker.public.safe',
        'raw terminal /Users/christopher/output.log',
      ],
      entries: [
        {
          ...readyCiMode,
          modeRef: 'interaction-mode.public.safe',
          promptAvailabilityRefs: [
            'prompt-availability.public.safe',
            'prompt text private',
          ],
          schemaRefs: [
            'schema.public.safe',
            'structured output payload private',
          ],
          structuredOutputRefs: [
            'structured-output.public.safe',
            'private output /Users/christopher/output.json',
          ],
          terminalCapabilityRefs: [
            'terminal-capability.public.safe',
            'terminal capture content private',
          ],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.schemaRefs).toEqual(['schema.public.safe'])
    expect(view.entries[0]?.structuredOutputRefs).toEqual([
      'structured-output.public.safe',
    ])
    expect(view.entries[0]?.terminalCapabilityRefs).toEqual([
      'terminal-capability.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-accessibility-non-interactive-blocker:work.public.work_1:unsafe-accessibility-non-interactive-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw terminal')
    expect(payload).not.toContain('prompt text')
    expect(payload).not.toContain('structured output payload')
    expect(payload).not.toContain('private output')
    expect(payload).not.toContain('terminal capture content')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      accessibilityNonInteractiveEvidence: {
        entries: [readyCiMode],
        generatedAt: '2026-06-18T12:01:00.000Z',
        snapshotRef: 'accessibility-non-interactive-snapshot.public.work_2',
        versionRef: 'accessibility-non-interactive-version.public.v2',
      },
      generatedAt: '2026-06-18T12:00:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeAccessibilityNonInteractiveInput(work)).toEqual({
      entries: [readyCiMode],
      generatedAt: '2026-06-18T12:01:00.000Z',
      snapshotRef: 'accessibility-non-interactive-snapshot.public.work_2',
      versionRef: 'accessibility-non-interactive-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
