import { Effect, Match as M, Option } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadMulletBootstrap,
  Message,
  SucceededLoadMulletBootstrap,
} from '../message'
import { Model } from '../model'
import { type UpdateReturn } from '../transition'
import {
  MulletBootstrapFailed,
  MulletBootstrapLoaded,
  MulletBootstrapLoading,
  MulletBootstrapResponse,
} from './model'
import {
  assumptionsForTemplate,
  defaultMulletSensitivityAxisId,
  mulletScenarioTemplateIds,
  mulletSensitivityAxisIds,
  type MulletScenarioTemplateId,
  type MulletSensitivityAxisId,
  updateMulletAssumption,
} from './workbench'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const LoadMulletBootstrap = Command.define(
  'LoadMulletBootstrap',
  SucceededLoadMulletBootstrap,
  FailedLoadMulletBootstrap,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.mullet.bootstrap.load',
      request: '/api/mullet/bootstrap',
      schema: MulletBootstrapResponse,
    })

    return SucceededLoadMulletBootstrap({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadMulletBootstrap({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const updateMullet = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadMulletBootstrap: () => [
        evo(model, {
          mullet: mullet =>
            evo(mullet, { bootstrap: () => MulletBootstrapLoading() }),
        }),
        [LoadMulletBootstrap()],
        Option.none(),
      ],
      SucceededLoadMulletBootstrap: ({ response }) => [
        evo(model, {
          mullet: mullet =>
            evo(mullet, {
              bootstrap: () => MulletBootstrapLoaded({ response }),
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadMulletBootstrap: ({ error }) => [
        evo(model, {
          mullet: mullet =>
            evo(mullet, {
              bootstrap: () => MulletBootstrapFailed({ error }),
            }),
        }),
        [],
        Option.none(),
      ],
      SelectedMulletScenarioTemplate: ({ templateId }) => {
        const selectedTemplateId = templateIdFromInput(templateId)

        return [
          evo(model, {
            mullet: mullet =>
              evo(mullet, {
                assumptions: () => [...assumptionsForTemplate(selectedTemplateId)],
                selectedSensitivityAxisId: () => defaultMulletSensitivityAxisId,
                selectedTemplateId: () => selectedTemplateId,
              }),
          }),
          [],
          Option.none(),
        ]
      },
      SelectedMulletSensitivityAxis: ({ axisId }) => [
        evo(model, {
          mullet: mullet =>
            evo(mullet, {
              selectedSensitivityAxisId: () => sensitivityAxisFromInput(axisId),
            }),
        }),
        [],
        Option.none(),
      ],
      UpdatedMulletAssumption: ({ assumptionId, field, value }) => [
        evo(model, {
          mullet: mullet =>
            evo(mullet, {
              assumptions: assumptions =>
                updateMulletAssumption(assumptions, {
                  assumptionId,
                  field,
                  value,
                }),
            }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )

const templateIdFromInput = (value: string): MulletScenarioTemplateId =>
  mulletScenarioTemplateIds.includes(value as MulletScenarioTemplateId)
    ? (value as MulletScenarioTemplateId)
    : mulletScenarioTemplateIds[0]

const sensitivityAxisFromInput = (value: string): MulletSensitivityAxisId =>
  mulletSensitivityAxisIds.includes(value as MulletSensitivityAxisId)
    ? (value as MulletSensitivityAxisId)
    : mulletSensitivityAxisIds[0]
