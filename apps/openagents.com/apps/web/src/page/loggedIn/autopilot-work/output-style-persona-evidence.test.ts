import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeOutputStylePersonaInput,
  projectForgeOutputStylePersonaEvidence,
} from './output-style-persona-evidence'

const baseInput = {
  generatedAt: '2026-06-18T08:00:00.000Z',
  snapshotRef: 'output-style-snapshot.public.work_1',
  versionRef: 'output-style-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyStylePolicy = {
  citationRequirementRefs: ['citation-policy.public.receipts_required'],
  domainMode: 'review' as const,
  evidenceRequirementRefs: ['evidence-policy.public.review_claims'],
  finalAnswerExpectationRefs: ['final-answer.public.review_findings_first'],
  formattingRefs: ['formatting.public.markdown_bullets'],
  freshness: 'fresh' as const,
  productDefaultRefs: ['product-style.public.engineering_plain'],
  projectConstraintRefs: ['project-style.public.openagents'],
  safetyPolicyRefs: ['safety-policy.public.style_cannot_override'],
  status: 'ready' as const,
  stylePolicyRef: 'style-policy.public.review',
  toolAuthorityBoundaryRefs: ['tool-authority.public.style_no_change'],
  userPreferenceRefs: ['style-preference.public.concise'],
  verbosity: 'concise' as const,
}

describe('Forge output style and persona evidence projection', () => {
  test('projects output style policy as refs-only non-authoritative state', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [readyStylePolicy],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      accessibilityPolicies: 0,
      conflicts: 0,
      overrides: 0,
      policies: 1,
      ready: 1,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      formatterExecutionAuthority: false,
      hiddenChainAccessAuthority: false,
      instructionMutationAuthority: false,
      managedPolicyMutationAuthority: false,
      outputRewriteAuthority: false,
      personaInstallAuthority: false,
      privateDataReadAuthority: false,
      productClaimMutationAuthority: false,
      promptMutationAuthority: false,
      safetyPrivacyApprovalBypassAuthority: false,
      settlementAuthority: false,
      stylePreferenceWriteAuthority: false,
      toolAuthorityChangeAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing output style evidence as empty', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      generatedAt: '2026-06-18T08:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks ready policies missing preference product or constraint refs', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          projectConstraintRefs: [],
          userPreferenceRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:style-policy-resolution-evidence-missing:style-policy.public.review',
    )
  })

  test('blocks shaped modes missing final answer or formatting refs', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          finalAnswerExpectationRefs: [],
          formattingRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:mode-output-shape-evidence-missing:style-policy.public.review',
    )
  })

  test('blocks accessibility and persona precedence without conflict resolution refs', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          accessibilityRefs: ['accessibility.public.screen_reader_plain_text'],
          personaConstraintRefs: ['persona.public.decorative_voice'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:accessibility-persona-precedence-missing:style-policy.public.review',
    )
  })

  test('blocks persona overrides without conflict resolution refs when policy wins', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          overrideRefs: ['style-override.public.single_turn'],
          status: 'conflicted',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:style-conflict-resolution-missing:style-policy.public.review',
    )
  })

  test('blocks disallowed capability claims without receipt or evidence refs', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          claimReceiptRefs: [],
          disallowedClaimRefs: ['claim.public.remote_execution_live'],
          evidenceRequirementRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:disallowed-claim-evidence-missing:style-policy.public.review',
    )
  })

  test('blocks stale output style evidence', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          ...readyStylePolicy,
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:stale-output-style-evidence:style-policy.public.review',
    )
  })

  test('surfaces planned style policies without presenting them as ready', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      entries: [
        {
          domainMode: 'support',
          status: 'planned',
          stylePolicyRef: 'style-policy.public.future_voice',
          verbosity: 'normal',
        },
      ],
    })

    expect(view.status).toBe('planned')
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks populated output style entries without snapshot refs', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      entries: [readyStylePolicy],
      generatedAt: '2026-06-18T08:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.no_snapshot:missing-output-style-persona-snapshot-ref',
    )
  })

  test('omits unsafe private style material before projection', () => {
    const view = projectForgeOutputStylePersonaEvidence({
      ...baseInput,
      blockerRefs: [
        'style-blocker.public.safe',
        'raw prompt /Users/christopher/prompt.md',
      ],
      entries: [
        {
          ...readyStylePolicy,
          audienceRefs: ['audience.public.engineering', 'private user preference'],
          claimReceiptRefs: ['claim-receipt.public.safe', 'capability claim private'],
          conflictResolutionRefs: ['style-conflict.public.safe'],
          disallowedClaimRefs: ['claim.public.safe'],
          formattingRefs: ['formatting.public.safe', 'raw output body secret'],
          overrideRefs: [
            'style-override.public.safe',
            'secret-bearing-override password',
          ],
          personaConstraintRefs: ['persona.public.safe', 'persona text hidden chain'],
          projectConstraintRefs: ['project-style.public.safe', 'project instruction private'],
          styleAuditRefs: ['style-audit.public.safe', 'hidden chain state private'],
          stylePolicyRef: 'style-policy.public.safe',
          userPreferenceRefs: ['style-preference.public.safe'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.audienceRefs).toEqual(['audience.public.engineering'])
    expect(view.entries[0]?.claimReceiptRefs).toEqual(['claim-receipt.public.safe'])
    expect(view.entries[0]?.formattingRefs).toEqual(['formatting.public.safe'])
    expect(view.entries[0]?.overrideRefs).toEqual(['style-override.public.safe'])
    expect(view.entries[0]?.personaConstraintRefs).toEqual(['persona.public.safe'])
    expect(view.entries[0]?.projectConstraintRefs).toEqual([
      'project-style.public.safe',
    ])
    expect(view.entries[0]?.styleAuditRefs).toEqual(['style-audit.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-output-style-persona-blocker:work.public.work_1:unsafe-output-style-persona-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('private user preference')
    expect(payload).not.toContain('capability claim')
    expect(payload).not.toContain('raw output body')
    expect(payload).not.toContain('secret-bearing-override')
    expect(payload).not.toContain('persona text')
    expect(payload).not.toContain('project instruction')
    expect(payload).not.toContain('hidden chain')
    expect(payload).not.toContain('password')
    expect(payload).not.toContain('secret')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T08:00:00.000Z',
      outputStylePersonaEvidence: {
        entries: [readyStylePolicy],
        generatedAt: '2026-06-18T08:01:00.000Z',
        snapshotRef: 'output-style-snapshot.public.work_2',
        versionRef: 'output-style-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeOutputStylePersonaInput(work)).toEqual({
      entries: [readyStylePolicy],
      generatedAt: '2026-06-18T08:01:00.000Z',
      snapshotRef: 'output-style-snapshot.public.work_2',
      versionRef: 'output-style-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
