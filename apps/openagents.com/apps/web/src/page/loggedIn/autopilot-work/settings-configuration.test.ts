import { describe, expect, test } from 'vitest'

import {
  buildForgeSettingsConfigurationInput,
  projectForgeSettingsConfiguration,
} from './settings-configuration'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:20:00.000Z',
  snapshotRef: 'settings-configuration-snapshot.public.work_1',
  versionRef: 'settings-configuration-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge settings configuration projection', () => {
  test('projects public settings evidence as refs-only non-authoritative state', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      entries: [
        {
          defaultRefs: ['setting-default.public.model_alias'],
          effectiveValueRefs: ['setting-effective.public.model_alias'],
          freshness: 'fresh',
          policyRefs: ['policy.public.settings.model_alias'],
          redactionClass: 'public',
          scopeRefs: ['setting-scope.public.workspace'],
          settingRef: 'setting.public.model_alias',
          sourceRefs: ['source.public.settings.workspace'],
          state: 'enabled',
          validationRefs: ['validation.public.settings.model_alias'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      defaulted: 0,
      enabled: 1,
      overridden: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      credentialAuthority: false,
      deploymentAuthority: false,
      effectiveConfigMutationAuthority: false,
      fileReadAuthority: false,
      publicClaimAuthority: false,
      settingsActivationAuthority: false,
      settingsReadAuthority: false,
      settingsWriteAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolExecutionAuthority: false,
      toolRoutingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing settings configuration state as empty', () => {
    const view = projectForgeSettingsConfiguration({
      generatedAt: '2026-06-17T23:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale settings evidence', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          policyRefs: ['policy.public.settings.default'],
          settingRef: 'setting.public.stale',
          state: 'enabled',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.work_1:stale-settings-evidence:setting.public.stale',
    )
  })

  test('blocks enabled or overridden settings without policy refs', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          settingRef: 'setting.public.no_policy',
          state: 'overridden',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.work_1:settings-policy-ref-missing:setting.public.no_policy',
    )
  })

  test('blocks effective values without validation refs', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      entries: [
        {
          effectiveValueRefs: ['setting-effective.public.no_validation'],
          freshness: 'fresh',
          policyRefs: ['policy.public.settings.default'],
          settingRef: 'setting.public.no_validation',
          state: 'enabled',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.work_1:effective-value-validation-ref-missing:setting.public.no_validation',
    )
  })

  test('blocks private and local settings without redaction refs', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          policyRefs: ['policy.public.settings.private_ref'],
          redactionClass: 'private_ref',
          settingRef: 'setting.public.private_ref',
          state: 'enabled',
          validationRefs: ['validation.public.settings.private_ref'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.work_1:settings-redaction-ref-missing:setting.public.private_ref',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeSettingsConfiguration({
      generatedAt: '2026-06-17T23:20:00.000Z',
      entries: [
        {
          freshness: 'fresh',
          policyRefs: ['policy.public.settings.default'],
          settingRef: 'setting.public.no_snapshot',
          state: 'defaulted',
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.no_snapshot:missing-settings-configuration-snapshot-ref',
    )
  })

  test('omits unsafe private settings material before projection', () => {
    const view = projectForgeSettingsConfiguration({
      ...baseInput,
      blockerRefs: [
        'settings-blocker.public.safe',
        'raw settings /Users/christopher/settings.json',
      ],
      entries: [
        {
          defaultRefs: ['setting-default.public.safe'],
          effectiveValueRefs: [
            'setting-effective.public.safe',
            'raw value /Users/christopher/value.json',
          ],
          freshness: 'fresh',
          overrideRefs: ['setting-override.public.safe', 'private setting token'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          redactionClass: 'private_ref',
          redactionRefs: ['redaction.public.safe'],
          scopeRefs: ['setting-scope.public.safe'],
          settingRef: 'setting.public.safe',
          sourceRefs: ['source.public.safe', 'private config /Users/christopher/config'],
          state: 'overridden',
          validationRefs: ['validation.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.effectiveValueRefs).toEqual([
      'setting-effective.public.safe',
    ])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-settings-configuration-blocker:work.public.work_1:unsafe-settings-configuration-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw settings')
    expect(payload).not.toContain('raw value')
    expect(payload).not.toContain('private setting')
    expect(payload).not.toContain('private config')
    expect(payload).not.toContain('bearer token')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T23:21:00.000Z',
      settingsConfiguration: {
        entries: [
          {
            freshness: 'fresh',
            policyRefs: ['policy.public.work_2'],
            settingRef: 'setting.public.work_2',
            state: 'defaulted',
          },
        ],
        snapshotRef: 'settings-configuration-snapshot.public.work_2',
        versionRef: 'settings-configuration-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeSettingsConfigurationInput(work)).toEqual({
      entries: [
        {
          freshness: 'fresh',
          policyRefs: ['policy.public.work_2'],
          settingRef: 'setting.public.work_2',
          state: 'defaulted',
        },
      ],
      generatedAt: '2026-06-17T23:21:00.000Z',
      snapshotRef: 'settings-configuration-snapshot.public.work_2',
      versionRef: 'settings-configuration-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
