import { describe, expect, test } from 'vitest'

import {
  type AuthBootstrap,
  completedOnboardingStatus,
  emptyBillingSummary,
} from '../../../domain/session'
import { MulletRoute } from '../../../route'
import {
  RequestedLoadMulletBootstrap,
  SelectedMulletScenarioTemplate,
  SelectedMulletSensitivityAxis,
  UpdatedMulletAssumption,
} from '../message'
import { init } from '../model'
import { updateMullet } from './transitions'

const auth: AuthBootstrap = {
  session: {
    avatarUrl: 'https://avatars.githubusercontent.com/u/14167547?v=4',
    email: 'chris@openagents.com',
    login: 'chris',
    name: 'Christopher David',
    userId: 'github:14167547',
  },
  teams: [],
  billing: emptyBillingSummary(),
  onboarding: completedOnboardingStatus(),
  isAdmin: true,
}

describe('mullet transitions', () => {
  test('updates scenario template, assumptions, sensitivity focus, and bootstrap state', () => {
    const base = init(MulletRoute(), auth)

    expect(base.mullet.selectedTemplateId).toBe('tinybox_shc_power')

    const [facilityModel] = updateMullet(
      base,
      SelectedMulletScenarioTemplate({
        templateId: 'facility_100mw_80_20',
      }),
    )

    expect(facilityModel.mullet.selectedTemplateId).toBe(
      'facility_100mw_80_20',
    )
    expect(
      facilityModel.mullet.assumptions.find(
        assumption => assumption.id === 'facility.capacityMw',
      )?.draftValue,
    ).toBe('100')

    const [editedModel] = updateMullet(
      facilityModel,
      UpdatedMulletAssumption({
        assumptionId: 'power.electricityUsdPerMwh',
        field: 'value',
        value: '52',
      }),
    )

    expect(
      editedModel.mullet.assumptions.find(
        assumption => assumption.id === 'power.electricityUsdPerMwh',
      )?.draftValue,
    ).toBe('52')

    const [provenanceModel] = updateMullet(
      editedModel,
      UpdatedMulletAssumption({
        assumptionId: 'power.electricityUsdPerMwh',
        field: 'provenance',
        value: 'measured',
      }),
    )

    expect(
      provenanceModel.mullet.assumptions.find(
        assumption => assumption.id === 'power.electricityUsdPerMwh',
      )?.state,
    ).toBe('measured')

    const [sensitivityModel] = updateMullet(
      provenanceModel,
      SelectedMulletSensitivityAxis({ axisId: 'raw_gpu_rate' }),
    )

    expect(sensitivityModel.mullet.selectedSensitivityAxisId).toBe(
      'raw_gpu_rate',
    )

    const [loadingModel, commands] = updateMullet(
      sensitivityModel,
      RequestedLoadMulletBootstrap(),
    )

    expect(loadingModel.mullet.bootstrap._tag).toBe('MulletBootstrapLoading')
    expect(commands.map(command => command.name)).toEqual([
      'LoadMulletBootstrap',
    ])
  })
})
