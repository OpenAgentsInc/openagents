import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintContextPack,
  blueprintContextPackProjection,
  BlueprintContextPack as BlueprintContextPackSchema,
  blueprintSourceCanProject,
} from './source-context'

const contextPack: BlueprintContextPack = {
  createdAt: '2026-06-05T00:00:00.000Z',
  customerSafeProjection: true,
  dataClassification: 'customer',
  excludedContextRefs: ['email.raw_customer_thread_1'],
  id: 'context_pack.site_revision_2',
  includedContextRefs: [
    'order.otec',
    'exa_brief.otec_sources',
    'repo.openagents_public',
  ],
  publicSafeProjection: true,
  sourceAuthorities: [
    {
      classificationCaveatRef: 'classification.public_source',
      confidence: 'high',
      consentState: 'public',
      customerSafe: true,
      dataClassification: 'public',
      excludedReasonRef: null,
      freshness: 'current',
      includedInContext: true,
      publicSafe: true,
      publicSummaryRef: 'summary.exa_otec_public',
      sourceKind: 'exa_brief',
      sourceRef: 'exa_brief.otec_sources',
      trustTier: 'reviewed',
    },
    {
      classificationCaveatRef: 'classification.customer_order',
      confidence: 'high',
      consentState: 'customer_provided',
      customerSafe: true,
      dataClassification: 'customer',
      excludedReasonRef: null,
      freshness: 'current',
      includedInContext: true,
      publicSafe: false,
      publicSummaryRef: 'summary.order_public_safe',
      sourceKind: 'order',
      sourceRef: 'order.otec',
      trustTier: 'verified',
    },
    {
      classificationCaveatRef: 'classification.private_email',
      confidence: 'medium',
      consentState: 'internal_only',
      customerSafe: false,
      dataClassification: 'private',
      excludedReasonRef: 'excluded.raw_private_email',
      freshness: 'recent',
      includedInContext: false,
      publicSafe: false,
      publicSummaryRef: null,
      sourceKind: 'email',
      sourceRef: 'email.raw_customer_thread_1',
      trustTier: 'reviewed',
    },
  ],
  trustTier: 'reviewed',
  updatedAt: '2026-06-05T00:00:00.000Z',
}

describe('Blueprint Source Authority and Context Pack schemas', () => {
  test('decodes context packs with source authorities', () => {
    expect(S.decodeUnknownSync(BlueprintContextPackSchema)(contextPack)).toEqual(
      contextPack,
    )
  })

  test('projects only public-safe context to public surfaces', () => {
    expect(blueprintContextPackProjection(contextPack, 'public')).toEqual({
      dataClassification: 'customer',
      excludedContextCount: 1,
      id: 'context_pack.site_revision_2',
      publicSafeProjection: true,
      sourceRefs: ['exa_brief.otec_sources'],
      trustTier: 'reviewed',
    })
  })

  test('projects customer-safe context to customer surfaces without raw email', () => {
    const privateEmailSource = contextPack.sourceAuthorities.find(
      source => source.sourceRef === 'email.raw_customer_thread_1',
    )

    expect(blueprintContextPackProjection(contextPack, 'customer').sourceRefs).toEqual(
      ['exa_brief.otec_sources', 'order.otec'],
    )
    expect(privateEmailSource).toBeDefined()
    expect(blueprintSourceCanProject(privateEmailSource!, 'customer')).toBe(false)
  })
})
