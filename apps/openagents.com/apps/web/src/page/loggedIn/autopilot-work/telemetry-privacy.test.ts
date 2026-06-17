import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeTelemetryPrivacyInput,
  projectForgeTelemetryPrivacy,
} from './telemetry-privacy'

const baseInput = {
  generatedAt: '2026-06-18T03:00:00.000Z',
  modeRefs: ['telemetry-mode.public.local_only'],
  policyRefs: ['policy.public.telemetry.redacted'],
  privacyFilterRefs: ['privacy-filter.public.public_safe_payloads'],
  redactionScanRefs: ['redaction-scan.public.telemetry.pass'],
  retentionRefs: ['retention.public.telemetry.30d'],
  sinkRefs: ['telemetry-sink.public.local'],
  snapshotRef: 'telemetry-privacy-snapshot.public.work_1',
  versionRef: 'telemetry-privacy-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyClass = {
  aggregateRefs: ['telemetry-aggregate.public.health'],
  classKind: 'health_events' as const,
  deliveryRefs: ['telemetry-delivery.public.local'],
  exportabilityRefs: ['telemetry-exportability.public.local_bundle'],
  freshness: 'fresh' as const,
  mode: 'local_only' as const,
  policyRefs: ['policy.public.telemetry.redacted'],
  privacyFilterRefs: ['privacy-filter.public.public_safe_payloads'],
  redactionScanRefs: ['redaction-scan.public.telemetry.pass'],
  retentionRefs: ['retention.public.telemetry.30d'],
  sinkRefs: ['telemetry-sink.public.local'],
  status: 'enabled' as const,
  telemetryRef: 'telemetry.public.health',
  visibilityRefs: ['visibility.public.local_only'],
}

describe('Forge telemetry and privacy projection', () => {
  test('projects telemetry privacy evidence as refs-only non-authoritative state', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [readyClass],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      disabled: 0,
      enabled: 1,
      failed: 0,
      product: 0,
      stale: 0,
      telemetryClasses: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      diagnosticExportAuthority: false,
      privacyFilterBypassAuthority: false,
      publicClaimAuthority: false,
      retentionDeletionAuthority: false,
      settlementAuthority: false,
      sinkActivationAuthority: false,
      telemetryEmitAuthority: false,
      telemetryModeWriteAuthority: false,
      usageBillingMutationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing telemetry state as empty', () => {
    const view = projectForgeTelemetryPrivacy({
      generatedAt: '2026-06-18T03:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.items).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale telemetry evidence', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [{ ...readyClass, freshness: 'stale', telemetryRef: 'telemetry.public.stale' }],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:stale-telemetry-evidence:telemetry.public.stale',
    )
  })

  test('blocks product telemetry when opted out', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      optOutRefs: ['telemetry-opt-out.public.user_1'],
      classes: [
        {
          ...readyClass,
          classKind: 'product_metrics',
          mode: 'product_improvement',
          telemetryRef: 'telemetry.public.product',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:product-telemetry-opted-out:telemetry.public.product',
    )
  })

  test('blocks enabled telemetry without sink and policy refs', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [
        {
          ...readyClass,
          policyRefs: [],
          sinkRefs: [],
          telemetryRef: 'telemetry.public.no_sink',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:enabled-telemetry-sink-policy-missing:telemetry.public.no_sink',
    )
  })

  test('blocks telemetry classes missing visibility retention or exportability refs', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [
        {
          ...readyClass,
          exportabilityRefs: [],
          retentionRefs: [],
          telemetryRef: 'telemetry.public.no_metadata',
          visibilityRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:telemetry-class-metadata-missing:telemetry.public.no_metadata',
    )
  })

  test('blocks diagnostic bundles without redaction and retention refs', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [
        {
          ...readyClass,
          diagnosticBundleRefs: ['diagnostic-bundle.public.local'],
          redactionScanRefs: [],
          retentionRefs: [],
          telemetryRef: 'telemetry.public.diagnostic',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:diagnostic-export-redaction-retention-missing:telemetry.public.diagnostic',
    )
  })

  test('blocks failed telemetry delivery without failure refs', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      classes: [
        {
          ...readyClass,
          status: 'failed',
          telemetryRef: 'telemetry.public.failed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:telemetry-delivery-failure-missing:telemetry.public.failed',
    )
  })

  test('blocks populated telemetry entries without snapshot refs', () => {
    const view = projectForgeTelemetryPrivacy({
      classes: [readyClass],
      generatedAt: '2026-06-18T03:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.no_snapshot:missing-telemetry-privacy-snapshot-ref',
    )
  })

  test('omits unsafe private telemetry material before projection', () => {
    const view = projectForgeTelemetryPrivacy({
      ...baseInput,
      blockerRefs: [
        'telemetry-blocker.public.safe',
        'raw telemetry /Users/christopher/telemetry.json',
      ],
      classes: [
        {
          ...readyClass,
          aggregateRefs: ['telemetry-aggregate.public.safe', 'raw prompt /Users/christopher/prompt.md'],
          diagnosticBundleRefs: ['diagnostic-bundle.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          privacyFilterRefs: ['privacy-filter.public.safe'],
          redactionScanRefs: ['redaction-scan.public.safe'],
          sinkRefs: ['telemetry-sink.public.safe', 'provider payload sk-private'],
          telemetryRef: 'telemetry.public.safe',
          visibilityRefs: ['visibility.public.safe', 'customer data private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.items[0]?.aggregateRefs).toEqual(['telemetry-aggregate.public.safe'])
    expect(view.items[0]?.sinkRefs).toEqual(['telemetry-sink.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-telemetry-privacy-blocker:work.public.work_1:unsafe-telemetry-privacy-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw telemetry')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('customer data')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T03:00:00.000Z',
      telemetryPrivacy: {
        classes: [readyClass],
        generatedAt: '2026-06-18T03:01:00.000Z',
        modeRefs: ['telemetry-mode.public.work_2'],
        snapshotRef: 'telemetry-privacy-snapshot.public.work_2',
        versionRef: 'telemetry-privacy-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeTelemetryPrivacyInput(work)).toEqual({
      classes: [readyClass],
      generatedAt: '2026-06-18T03:01:00.000Z',
      modeRefs: ['telemetry-mode.public.work_2'],
      snapshotRef: 'telemetry-privacy-snapshot.public.work_2',
      versionRef: 'telemetry-privacy-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
