import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_MODEL_CUSTODY_LEAD_GEN_CONFIG,
  decodeLeadGenRunPayload,
  leadGenRunRequest,
} from './autopilot-lead-gen-agent-definition'
import {
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS,
  lintBusinessOutreachClaims,
} from './business-outreach'
import {
  businessSourceKindForSourceRef,
  decodeBusinessSourceRef,
} from './business-source-attribution'

describe('RX-8 model-custody lead gen segment', () => {
  test('keeps the RX-8 source ref bounded and outbound-classified', () => {
    expect(decodeBusinessSourceRef('apollo-model-custody')).toEqual({
      sourceRef: 'apollo_model_custody',
    })
    expect(businessSourceKindForSourceRef('apollo_model_custody')).toBe(
      'outbound',
    )
  })

  test('registers the model-custody customer config in the LG-7 run payload', () => {
    const payload = decodeLeadGenRunPayload(
      leadGenRunRequest(OPENAGENTS_MODEL_CUSTODY_LEAD_GEN_CONFIG)
        .triggerPayload,
    )

    expect(payload.customerConfig).toMatchObject({
      analyzerConfigRef:
        'analyzer.agent_readiness.model_custody.own_your_ai.v1',
      sourceRef: 'apollo_model_custody',
      targetDiscoveryConfigRef:
        'target_discovery.openagents.model_custody.hand_approved.v1',
      templateFamilyRef:
        'template_family.lead_gen.model_custody_regulated.reactor_assessment.v1',
    })
    expect(payload.sendAuthority.allowed).toBe(false)
  })

  test('ships a claim-lint-clean regulated Reactor Assessment template variant', () => {
    const template = BUSINESS_OUTREACH_TEMPLATE_VERSIONS.find(
      item =>
        item.templateVersionRef ===
        'business.outreach.model_custody_regulated.reactor_assessment.v1',
    )

    expect(template).toBeDefined()
    expect(template?.segmentRef).toBe('model_custody_regulated')
    const copy = [
      template?.offerSentence,
      template?.proofPoint,
      template?.cta,
      template?.identificationOptOut,
    ].join('\n')
    expect(copy).toContain('Reactor Assessment')
    expect(copy).toContain('Friedberg/Mistral')
    expect(lintBusinessOutreachClaims(copy)).toEqual([])
    expect(copy).not.toMatch(/\b(HIPAA|sovereign|\$\s?\d)/i)
  })
})
