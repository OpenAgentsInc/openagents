import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeTipsEducationInput,
  projectForgeTipsEducationEvidence,
} from './tips-education-evidence'

const baseInput = {
  generatedAt: '2026-06-18T10:00:00.000Z',
  snapshotRef: 'tips-education-snapshot.public.work_1',
  versionRef: 'tips-education-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyCapabilityTip = {
  audienceRefs: ['education-audience.public.first_run'],
  capabilityRefs: ['capability.public.review_mode'],
  docsRefs: ['docs.public.review_mode'],
  freshness: 'fresh' as const,
  liveStateRefs: ['live-state.public.review_mode_ready'],
  scopeRefs: ['education-scope.public.current_run'],
  status: 'ready' as const,
  tipRef: 'tip.public.review_mode',
  topic: 'capability' as const,
  triggerRefs: ['tip-trigger.public.first_run'],
  versionRefs: ['tip-version.public.v1'],
}

describe('Forge tips and education evidence projection', () => {
  test('projects tips and education as refs-only non-authoritative state', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [readyCapabilityTip],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      caveats: 0,
      dismissed: 0,
      ready: 1,
      requiredWarnings: 0,
      stale: 0,
      tips: 1,
      unsupported: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      approvalPromptDismissalAuthority: false,
      capabilityEnablementAuthority: false,
      dismissalMutationAuthority: false,
      docsReadAuthority: false,
      helpSearchAuthority: false,
      paymentActivationAuthority: false,
      policyCaveatDismissalAuthority: false,
      productClaimMutationAuthority: false,
      providerActivationAuthority: false,
      settlementActivationAuthority: false,
      settlementAuthority: false,
      tipRenderingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing tips evidence as empty', () => {
    const view = projectForgeTipsEducationEvidence({
      generatedAt: '2026-06-18T10:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks capability tips missing live state or capability refs', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCapabilityTip,
          capabilityRefs: [],
          liveStateRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:capability-tip-live-state-missing:tip.public.review_mode',
    )
  })

  test('blocks dismissed required warnings', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          requiredWarningRefs: ['warning.public.approval_required'],
          status: 'dismissed',
          tipRef: 'tip.public.approval_warning',
          topic: 'approval',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:required-warning-dismissed:tip.public.approval_warning',
    )
  })

  test('blocks dismissed optional tips without dismissal receipt refs', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          status: 'dismissed',
          tipRef: 'tip.public.optional_shortcut',
          topic: 'command',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:optional-tip-dismissal-receipt-missing:tip.public.optional_shortcut',
    )
  })

  test('blocks payment provider payout and settlement education missing exact caveats', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          capabilityRefs: ['capability.public.payouts'],
          liveStateRefs: ['live-state.public.payouts_pending'],
          status: 'ready',
          tipRef: 'tip.public.payouts',
          topic: 'payout',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:payment-provider-caveat-missing:tip.public.payouts',
    )
  })

  test('blocks non-interactive education without documentation refs', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          nonInteractiveModeRefs: ['non-interactive.public.json'],
          status: 'ready',
          tipRef: 'tip.public.non_interactive',
          topic: 'workflow',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:non-interactive-doc-ref-missing:tip.public.non_interactive',
    )
  })

  test('blocks unsupported capability education', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          status: 'unsupported',
          tipRef: 'tip.public.future_workflow',
          topic: 'workflow',
          unsupportedClaimRefs: ['unsupported-claim.public.future_workflow'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:unsupported-capability-education:tip.public.future_workflow',
    )
  })

  test('blocks stale and expired tips', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      entries: [
        {
          ...readyCapabilityTip,
          freshness: 'expired',
          status: 'expired',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:stale-or-expired-tip:tip.public.review_mode',
    )
  })

  test('blocks populated tips without snapshot refs', () => {
    const view = projectForgeTipsEducationEvidence({
      entries: [readyCapabilityTip],
      generatedAt: '2026-06-18T10:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.no_snapshot:missing-tips-education-snapshot-ref',
    )
  })

  test('omits unsafe private tips material before projection', () => {
    const view = projectForgeTipsEducationEvidence({
      ...baseInput,
      blockerRefs: [
        'tips-blocker.public.safe',
        'raw tip copy /Users/christopher/tip.md',
      ],
      entries: [
        {
          ...readyCapabilityTip,
          caveatRefs: ['caveat.public.safe', 'payment payload private'],
          docsRefs: ['docs.public.safe', 'docs content secret'],
          helpTopicRefs: ['help-topic.public.safe', 'secret-bearing-help payload'],
          requiredWarningRefs: ['warning.public.safe'],
          tipRef: 'tip.public.safe',
          triggerRefs: ['tip-trigger.public.safe', 'raw run data private'],
          unsupportedClaimRefs: ['unsupported-claim.public.safe'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.caveatRefs).toEqual(['caveat.public.safe'])
    expect(view.entries[0]?.docsRefs).toEqual(['docs.public.safe'])
    expect(view.entries[0]?.helpTopicRefs).toEqual(['help-topic.public.safe'])
    expect(view.entries[0]?.triggerRefs).toEqual(['tip-trigger.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-tips-education-blocker:work.public.work_1:unsafe-tips-education-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw tip copy')
    expect(payload).not.toContain('payment payload')
    expect(payload).not.toContain('docs content')
    expect(payload).not.toContain('secret-bearing-help')
    expect(payload).not.toContain('raw run data')
    expect(payload).not.toContain('secret')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T10:00:00.000Z',
      tipsEducationEvidence: {
        entries: [readyCapabilityTip],
        generatedAt: '2026-06-18T10:01:00.000Z',
        snapshotRef: 'tips-education-snapshot.public.work_2',
        versionRef: 'tips-education-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeTipsEducationInput(work)).toEqual({
      entries: [readyCapabilityTip],
      generatedAt: '2026-06-18T10:01:00.000Z',
      snapshotRef: 'tips-education-snapshot.public.work_2',
      versionRef: 'tips-education-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
