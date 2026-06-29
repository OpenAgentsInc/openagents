import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeThemeVisualInput,
  projectForgeThemeVisualEvidence,
} from './theme-visual-evidence'

const baseInput = {
  generatedAt: '2026-06-18T11:00:00.000Z',
  snapshotRef: 'theme-visual-snapshot.public.work_1',
  versionRef: 'theme-visual-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyTheme = {
  contrastCheckRefs: ['contrast.public.status_pass'],
  crossSurfaceRefs: [
    'surface.public.terminal',
    'surface.public.web',
    'surface.public.mobile',
    'surface.public.operator',
  ],
  freshness: 'fresh' as const,
  highContrastRefs: ['high-contrast.public.available'],
  monochromeRefs: ['monochrome.public.labels_icons'],
  reducedMotionRefs: ['motion.public.reduced'],
  runtimeReceiptRefs: ['runtime-receipt.public.success_state'],
  status: 'ready' as const,
  statusIconRefs: ['status-icon.public.success'],
  statusLabelRefs: ['status-label.public.success'],
  statusVisualRefs: ['status-visual.public.success_green'],
  surface: 'web' as const,
  themeRef: 'theme.public.default',
  tokenRefs: ['theme-token.public.roles'],
  warningPreservationRefs: ['warning-preservation.public.narrow_width'],
}

describe('Forge theme and visual design evidence projection', () => {
  test('projects theme visual evidence as refs-only non-authoritative state', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [readyTheme],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      highContrast: 1,
      managed: 0,
      ready: 1,
      reducedMotion: 1,
      stale: 0,
      surfaces: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      cssInjectionAuthority: false,
      managedPolicyMutationAuthority: false,
      productClaimMutationAuthority: false,
      preferenceWriteAuthority: false,
      remoteThemeExecutionAuthority: false,
      rendererMutationAuthority: false,
      runtimeStatusMutationAuthority: false,
      settlementAuthority: false,
      themeInstallAuthority: false,
      visualSnapshotAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing theme evidence as empty', () => {
    const view = projectForgeThemeVisualEvidence({
      generatedAt: '2026-06-18T11:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks success visuals without runtime receipt refs', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          runtimeReceiptRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:success-visual-runtime-receipt-missing:theme.public.default',
    )
  })

  test('blocks warning visuals missing labels icons contrast or monochrome refs', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          contrastCheckRefs: [],
          monochromeRefs: [],
          statusIconRefs: [],
          statusLabelRefs: [],
          statusVisualRefs: ['status-visual.public.warning'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:warning-visual-accessibility-evidence-missing:theme.public.default',
    )
  })

  test('blocks managed themes without high contrast evidence', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          highContrastRefs: [],
          managedPolicyRefs: ['managed-theme.public.high_contrast_required'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:managed-theme-high-contrast-evidence-missing:theme.public.default',
    )
  })

  test('blocks dense warning visuals without preservation refs', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          densityRefs: ['density.public.compact'],
          statusVisualRefs: ['status-visual.public.danger'],
          warningPreservationRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:critical-warning-preservation-missing:theme.public.default',
    )
  })

  test('blocks ready themes missing cross-surface consistency refs', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          crossSurfaceRefs: ['surface.public.web'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:cross-surface-visual-consistency-missing:theme.public.default',
    )
  })

  test('blocks stale theme evidence', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      entries: [
        {
          ...readyTheme,
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:stale-theme-visual-evidence:theme.public.default',
    )
  })

  test('blocks populated theme entries without snapshot refs', () => {
    const view = projectForgeThemeVisualEvidence({
      entries: [readyTheme],
      generatedAt: '2026-06-18T11:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.no_snapshot:missing-theme-visual-snapshot-ref',
    )
  })

  test('omits unsafe private theme material before projection', () => {
    const view = projectForgeThemeVisualEvidence({
      ...baseInput,
      blockerRefs: [
        'theme-blocker.public.safe',
        'executable-theme /Users/christopher/theme.js',
      ],
      entries: [
        {
          ...readyTheme,
          contrastCheckRefs: ['contrast.public.safe', 'raw css /Users/christopher/style.css'],
          snapshotRefs: ['visual-snapshot.public.safe', 'visual snapshot content private'],
          statusVisualRefs: [
            'status-visual.public.safe',
            'unsupported green claim private',
          ],
          themeRef: 'theme.public.safe',
          tokenRefs: ['theme-token.public.safe', 'plugin theme code private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.contrastCheckRefs).toEqual(['contrast.public.safe'])
    expect(view.entries[0]?.snapshotRefs).toEqual(['visual-snapshot.public.safe'])
    expect(view.entries[0]?.tokenRefs).toEqual(['theme-token.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-theme-visual-blocker:work.public.work_1:unsafe-theme-visual-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('executable-theme')
    expect(payload).not.toContain('raw css')
    expect(payload).not.toContain('visual snapshot content')
    expect(payload).not.toContain('unsupported green claim')
    expect(payload).not.toContain('plugin theme code')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T11:00:00.000Z',
      themeVisualEvidence: {
        entries: [readyTheme],
        generatedAt: '2026-06-18T11:01:00.000Z',
        snapshotRef: 'theme-visual-snapshot.public.work_2',
        versionRef: 'theme-visual-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeThemeVisualInput(work)).toEqual({
      entries: [readyTheme],
      generatedAt: '2026-06-18T11:01:00.000Z',
      snapshotRef: 'theme-visual-snapshot.public.work_2',
      versionRef: 'theme-visual-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
