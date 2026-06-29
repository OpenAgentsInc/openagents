import { describe, expect, test } from 'vitest'

import { OnboardingRoute } from '../../../route'
import {
  SelectedOnboardingRepository,
  SkippedOnboardingRepository,
  ToggledOnboardingCoupon,
  UpdatedOnboardingCouponCode,
  UpdatedOnboardingFundingAmount,
} from '../message'
import { init } from '../model'
import { update } from '../update'

describe('onboarding flow', () => {
  test('updates funding amount from the slider', () => {
    const [model, commands] = update(
      init(OnboardingRoute()),
      UpdatedOnboardingFundingAmount({ value: '100' }),
    )

    expect(model.onboarding.fundingAmount).toBe(100)
    expect(model.onboarding.step).toBe('funding')
    expect(commands).toHaveLength(0)
  })

  test('clamps funding to the five dollar minimum', () => {
    const [model] = update(
      init(OnboardingRoute()),
      UpdatedOnboardingFundingAmount({ value: '1' }),
    )

    expect(model.onboarding.fundingAmount).toBe(5)
  })

  test('moves repository selection into funding', () => {
    const [model] = update(
      init(OnboardingRoute()),
      SelectedOnboardingRepository({ repository: 'openagents/cloud' }),
    )

    expect(model.onboarding.selectedRepository).toBe('openagents/cloud')
    expect(model.onboarding.step).toBe('funding')
  })

  test('lets users skip repository selection', () => {
    const [model] = update(
      init(OnboardingRoute()),
      SkippedOnboardingRepository(),
    )

    expect(model.onboarding.selectedRepository).toBe('')
    expect(model.onboarding.step).toBe('funding')
  })

  test('opens and updates coupon code entry', () => {
    const [opened] = update(init(OnboardingRoute()), ToggledOnboardingCoupon())
    const [updated] = update(
      opened,
      UpdatedOnboardingCouponCode({ value: 'OPENAGENTS-10' }),
    )

    expect(opened.onboarding.isCouponOpen).toBe(true)
    expect(updated.onboarding.isCouponOpen).toBe(true)
    expect(updated.onboarding.couponCode).toBe('OPENAGENTS-10')
  })
})
