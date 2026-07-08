import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedGenerateImage,
  Message,
  SucceededGenerateImage,
} from '../message'
import {
  GenerateImageResponse,
  ImageGenerationFailed,
  ImageGenerationSubmitting,
  ImageGenerationSucceeded,
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const GenerateImage = Command.define(
  'GenerateImage',
  {
    aspectRatio: S.String,
    count: S.Number,
    imageSize: S.String,
    model: S.String,
    prompt: S.String,
    provider: S.String,
  },
  SucceededGenerateImage,
  FailedGenerateImage,
)(({ aspectRatio, count, imageSize, model, prompt, provider }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          aspectRatio,
          count,
          imageSize,
          model,
          prompt,
          provider,
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.images.generate',
      request: '/api/images/generate',
      schema: GenerateImageResponse,
    })

    return SucceededGenerateImage({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedGenerateImage({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const modelForProvider = (
  provider: string,
  current: Model['imageGenerationModel'],
): Model['imageGenerationModel'] => {
  if (provider === 'google-imagen' && current.startsWith('imagen-')) {
    return current
  }

  if (provider === 'google-imagen') {
    return 'imagen-4.0-generate-001'
  }

  if (provider === 'google-gemini' && current.startsWith('gemini-')) {
    return current
  }

  return 'gemini-3.5-flash-image'
}

const countFromInput = (value: string, fallback: number): number => {
  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed >= 1 && parsed <= 4
    ? parsed
    : fallback
}

export const updateImages = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      UpdatedImageGenerationPrompt: ({ value }) => [
        evo(model, { imageGenerationPrompt: () => value }),
        [],
        Option.none(),
      ],
      SelectedImageGenerationProvider: ({ provider }) => [
        evo(model, {
          imageGenerationModel: current => modelForProvider(provider, current),
          imageGenerationProvider: () => provider,
        }),
        [],
        Option.none(),
      ],
      SelectedImageGenerationModel: ({ model: modelId }) => [
        evo(model, { imageGenerationModel: () => modelId }),
        [],
        Option.none(),
      ],
      SelectedImageGenerationAspectRatio: ({ aspectRatio }) => [
        evo(model, { imageGenerationAspectRatio: () => aspectRatio }),
        [],
        Option.none(),
      ],
      SelectedImageGenerationImageSize: ({ imageSize }) => [
        evo(model, { imageGenerationImageSize: () => imageSize }),
        [],
        Option.none(),
      ],
      UpdatedImageGenerationCount: ({ value }) => [
        evo(model, {
          imageGenerationCount: count =>
            value.trim() === '' ? count : countFromInput(value, count),
        }),
        [],
        Option.none(),
      ],
      SubmittedImageGeneration: () => {
        const prompt = model.imageGenerationPrompt.trim()

        if (prompt.length < 3) {
          return [
            evo(model, {
              imageGeneration: () =>
                ImageGenerationFailed({
                  error: 'Enter a prompt with at least 3 characters.',
                }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, { imageGeneration: () => ImageGenerationSubmitting() }),
          [
            GenerateImage({
              aspectRatio: model.imageGenerationAspectRatio,
              count: model.imageGenerationCount,
              imageSize: model.imageGenerationImageSize,
              model: model.imageGenerationModel,
              prompt,
              provider: model.imageGenerationProvider,
            }),
          ],
          Option.none(),
        ]
      },
      SucceededGenerateImage: ({ response }) => [
        evo(model, {
          imageGeneration: () =>
            ImageGenerationSucceeded({ images: response.images }),
        }),
        [],
        Option.none(),
      ],
      FailedGenerateImage: ({ error }) => [
        evo(model, { imageGeneration: () => ImageGenerationFailed({ error }) }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
