import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeLocalizationBoundaryInput,
  projectForgeLocalizationBoundaryEvidence,
} from './localization-boundary-evidence'

const baseInput = {
  generatedAt: '2026-06-18T13:00:00.000Z',
  snapshotRef: 'localization-boundary-snapshot.public.work_1',
  versionRef: 'localization-boundary-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyLocalization = {
  catalogRefs: ['message-catalog.public.en_US'],
  catalogValidationRefs: ['catalog-validation.public.valid'],
  fallbackRefs: ['fallback.public.visible'],
  formatterRefs: ['formatter.public.date_number_currency'],
  freshness: 'fresh' as const,
  localePreferenceRefs: ['locale-preference.public.user_team_system'],
  localeRefs: ['locale.public.en-US'],
  localizationRef: 'localization.public.ui',
  scope: 'ui' as const,
  stableIdBoundaryRefs: ['stable-id-boundary.public.canonical_ids'],
  status: 'ready' as const,
}

describe('Forge localization boundary evidence projection', () => {
  test('projects localization boundary evidence as refs-only non-authoritative state', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [readyLocalization],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      catalogs: 1,
      fallbacks: 1,
      ready: 1,
      stableBoundaries: 1,
      stale: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      catalogExecutionAuthority: false,
      commandIdMutationAuthority: false,
      jsonSchemaMutationAuthority: false,
      localePreferenceWriteAuthority: false,
      localeRuntimeMutationAuthority: false,
      paymentLanguageMutationAuthority: false,
      permissionPromptMutationAuthority: false,
      publicReceiptMutationAuthority: false,
      settlementAuthority: false,
      toolIdMutationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing localization evidence as empty', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      generatedAt: '2026-06-18T13:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks ready localization without locale catalog validation fallback stable id or formatter refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          catalogRefs: [],
          catalogValidationRefs: [],
          fallbackRefs: [],
          formatterRefs: [],
          localePreferenceRefs: [],
          stableIdBoundaryRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:ready-localization-boundary-missing:localization.public.ui',
    )
  })

  test('blocks permission prompt localization without canonical action policy and permission id refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          localizationRef: 'localization.public.permission_prompt',
          permissionActionRefs: [],
          permissionIdStabilityRefs: [],
          permissionPolicyRefs: [],
          scope: 'permission_prompt',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:permission-prompt-stability-missing:localization.public.permission_prompt',
    )
  })

  test('blocks payment localization without precision review refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          localizationRef: 'localization.public.payment',
          paymentLanguageReviewRefs: [],
          scope: 'payment',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:payment-language-review-missing:localization.public.payment',
    )
  })

  test('blocks public receipt localization without language-stability refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          localizationRef: 'localization.public.receipt',
          publicReceiptStabilityRefs: [],
          scope: 'public_receipt',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:public-receipt-language-stability-missing:localization.public.receipt',
    )
  })

  test('blocks JSON schema localization without schema language-stability refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          jsonSchemaStabilityRefs: [],
          localizationRef: 'localization.public.json_schema',
          scope: 'json_schema',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:json-schema-language-stability-missing:localization.public.json_schema',
    )
  })

  test('blocks command localization without command and tool id stability refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          commandIdStabilityRefs: [],
          localizationRef: 'localization.public.command',
          scope: 'command',
          toolIdStabilityRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:command-tool-id-stability-missing:localization.public.command',
    )
  })

  test('blocks missing translations without visible fallback refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          fallbackRefs: [],
          missingTranslationRefs: ['missing-translation.public.help.es'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:missing-translation-fallback-missing:localization.public.ui',
    )
  })

  test('blocks stale localization evidence', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      entries: [
        {
          ...readyLocalization,
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:stale-localization-boundary-evidence:localization.public.ui',
    )
  })

  test('blocks populated localization entries without snapshot refs', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      entries: [readyLocalization],
      generatedAt: '2026-06-18T13:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.no_snapshot:missing-localization-boundary-snapshot-ref',
    )
  })

  test('omits unsafe private localization material before projection', () => {
    const view = projectForgeLocalizationBoundaryEvidence({
      ...baseInput,
      blockerRefs: [
        'localization-blocker.public.safe',
        'raw catalog /Users/christopher/catalog.json',
      ],
      entries: [
        {
          ...readyLocalization,
          catalogRefs: [
            'message-catalog.public.safe',
            'private catalog /Users/christopher/catalog.json',
          ],
          localizationRef: 'localization.public.safe',
          paymentLanguageReviewRefs: [
            'payment-language-review.public.safe',
            'payment payload private',
          ],
          stableIdBoundaryRefs: [
            'stable-id-boundary.public.safe',
            'translated identifier private',
          ],
          toolIdStabilityRefs: [
            'tool-id-stability.public.safe',
            'translation content private',
          ],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.catalogRefs).toEqual(['message-catalog.public.safe'])
    expect(view.entries[0]?.stableIdBoundaryRefs).toEqual([
      'stable-id-boundary.public.safe',
    ])
    expect(view.entries[0]?.toolIdStabilityRefs).toEqual([
      'tool-id-stability.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-localization-boundary-blocker:work.public.work_1:unsafe-localization-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw catalog')
    expect(payload).not.toContain('private catalog')
    expect(payload).not.toContain('payment payload')
    expect(payload).not.toContain('translated identifier')
    expect(payload).not.toContain('translation content')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T13:00:00.000Z',
      localizationBoundaryEvidence: {
        entries: [readyLocalization],
        generatedAt: '2026-06-18T13:01:00.000Z',
        snapshotRef: 'localization-boundary-snapshot.public.work_2',
        versionRef: 'localization-boundary-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeLocalizationBoundaryInput(work)).toEqual({
      entries: [readyLocalization],
      generatedAt: '2026-06-18T13:01:00.000Z',
      snapshotRef: 'localization-boundary-snapshot.public.work_2',
      versionRef: 'localization-boundary-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
