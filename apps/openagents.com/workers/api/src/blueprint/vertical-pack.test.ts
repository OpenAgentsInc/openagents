import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { BlueprintContextPack } from './schemas/source-context'
import {
  getVerticalPack,
  servicesBusinessVerticalPack,
  VerticalPack,
  VerticalPackEthicalMarketingPolicy,
  verticalPackRegistry,
} from './vertical-pack'

describe('services-business vertical pack', () => {
  test('the vertical pack decodes against its schema', () => {
    expect(S.decodeUnknownSync(VerticalPack)(servicesBusinessVerticalPack)).toEqual(
      servicesBusinessVerticalPack,
    )
  })

  test('the embedded Context Pack decodes as a BlueprintContextPack', () => {
    expect(
      S.decodeUnknownSync(BlueprintContextPack)(
        servicesBusinessVerticalPack.contextPack,
      ),
    ).toEqual(servicesBusinessVerticalPack.contextPack)
  })

  test('uses a generic vertical label, never a customer name', () => {
    expect(servicesBusinessVerticalPack.vertical).toBe('services_business')
    expect(servicesBusinessVerticalPack.id).toBe('vertical_pack.services_business')
  })

  test('the ethical-marketing policy block is present and decodes', () => {
    const policy = servicesBusinessVerticalPack.ethicalMarketingPolicy
    expect(
      S.decodeUnknownSync(VerticalPackEthicalMarketingPolicy)(policy),
    ).toEqual(policy)

    expect(policy.policyRef).toBe('policy.ethical_marketing.services_business')
    expect(policy.noFabricatedTestimonials).toBe(true)
    expect(policy.noFabricatedCredentials).toBe(true)
    expect(policy.noFakeUrgency).toBe(true)
    expect(policy.clarityOverHype).toBe(true)
    expect(policy.humanInLoopOnSensitiveSends).toBe(true)
    expect(policy.ruleRefs.length).toBeGreaterThan(0)
  })

  test('source authorities scope consent and projection posture', () => {
    const sources = servicesBusinessVerticalPack.contextPack.sourceAuthorities
    // At least one excluded raw-private source.
    expect(
      sources.some(s => !s.includedInContext && s.dataClassification === 'private'),
    ).toBe(true)
    // At least one public-safe source.
    expect(sources.some(s => s.publicSafe)).toBe(true)
    // At least one customer-consented source.
    expect(sources.some(s => s.consentState === 'customer_provided')).toBe(true)
  })

  test('registry lookup resolves the pack by id', () => {
    expect(getVerticalPack('vertical_pack.services_business')).toBe(
      servicesBusinessVerticalPack,
    )
    expect(verticalPackRegistry['vertical_pack.services_business']).toBe(
      servicesBusinessVerticalPack,
    )
  })

  test('registry lookup returns undefined for unknown ids', () => {
    expect(getVerticalPack('vertical_pack.unknown')).toBeUndefined()
  })
})
