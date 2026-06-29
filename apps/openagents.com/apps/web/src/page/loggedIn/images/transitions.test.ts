import { describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  completedOnboardingStatus,
} from '../../../domain/session'
import { ImagesRoute } from '../../../route'
import {
  SelectedImageGenerationProvider,
  SubmittedImageGeneration,
  SucceededGenerateImage,
  UpdatedImageGenerationPrompt,
} from '../message'
import { ImageGenerationFailed, ImageGenerationSucceeded, init } from '../model'
import { update } from '../update'

const auth = {
  ...authBootstrapFromSession({
    email: 'operator@openagents.com',
    name: 'Operator',
    userId: 'github:operator',
  }),
  onboarding: completedOnboardingStatus(),
  teams: [
    {
      id: 'team_openagents_core',
      members: [],
      name: 'OpenAgents Core Team',
      role: 'owner',
      slug: 'openagents-core-team',
    },
  ],
}

describe('image generation transitions', () => {
  test('submits the current image generation controls to the API command', () => {
    const model = init(ImagesRoute(), auth)
    const [withPrompt] = update(
      model,
      UpdatedImageGenerationPrompt({ value: 'Create an operator console.' }),
    )
    const [withProvider] = update(
      withPrompt,
      SelectedImageGenerationProvider({ provider: 'google-imagen' }),
    )
    const [submitting, commands] = update(
      withProvider,
      SubmittedImageGeneration(),
    )

    expect(submitting.imageGeneration._tag).toBe('ImageGenerationSubmitting')
    expect(commands.map(command => command.name)).toEqual(['GenerateImage'])
    expect(commands[0]?.args).toMatchObject({
      model: 'imagen-4.0-generate-001',
      prompt: 'Create an operator console.',
      provider: 'google-imagen',
    })
  })

  test('keeps short prompts as typed UI errors without calling the API', () => {
    const [model, commands] = update(
      init(ImagesRoute(), auth),
      SubmittedImageGeneration(),
    )

    expect(model.imageGeneration).toEqual(
      ImageGenerationFailed({
        error: 'Enter a prompt with at least 3 characters.',
      }),
    )
    expect(commands).toHaveLength(0)
  })

  test('stores generated image metadata after a successful response', () => {
    const [model] = update(
      init(ImagesRoute(), auth),
      SucceededGenerateImage({
        response: {
          images: [
            {
              byteLength: 68,
              createdAt: '2026-06-04T12:00:00.000Z',
              key: 'generated-images/users/2026-06-04/test.png',
              mimeType: 'image/png',
              model: 'gemini-2.5-flash-image',
              prompt: 'Create an operator console.',
              provider: 'google-gemini',
              url: '/api/images/generated-images%2Fusers%2F2026-06-04%2Ftest.png',
            },
          ],
        },
      }),
    )

    expect(model.imageGeneration).toEqual(
      ImageGenerationSucceeded({
        images: [
          {
            byteLength: 68,
            createdAt: '2026-06-04T12:00:00.000Z',
            key: 'generated-images/users/2026-06-04/test.png',
            mimeType: 'image/png',
            model: 'gemini-2.5-flash-image',
            prompt: 'Create an operator console.',
            provider: 'google-gemini',
            url: '/api/images/generated-images%2Fusers%2F2026-06-04%2Ftest.png',
          },
        ],
      }),
    )
  })
})
