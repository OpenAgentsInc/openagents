import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  Message,
  SelectedImageGenerationAspectRatio,
  SelectedImageGenerationImageSize,
  SelectedImageGenerationModel,
  SelectedImageGenerationProvider,
  SubmittedImageGeneration,
  UpdatedImageGenerationCount,
  UpdatedImageGenerationPrompt,
} from '../message'
import type {
  GeneratedImage,
  ImageGenerationAspectRatio,
  ImageGenerationImageSize,
  ImageGenerationModelId,
  ImageGenerationProvider,
  Model,
} from '../model'

const providerOptions = [
  { label: 'Gemini', value: 'google-gemini' },
  { label: 'Imagen', value: 'google-imagen' },
] as const

const geminiModels = ['gemini-3.5-flash-image'] as const

const imagenModels = [
  'imagen-4.0-generate-001',
  'imagen-4.0-fast-generate-001',
  'imagen-4.0-ultra-generate-001',
] as const

const aspectRatios = ['1:1', '3:4', '4:3', '9:16', '16:9'] as const
const imageSizes = ['512', '1K', '2K', '4K'] as const

const controlClass =
  'h-10 border border-[#2a2a2a] bg-black px-3 text-sm text-[#e8e8e8] outline-none focus:border-[#e8e8e8]'

const providerFromValue = (value: string): ImageGenerationProvider =>
  value === 'google-imagen' ? 'google-imagen' : 'google-gemini'

const modelFromValue = (
  value: string,
  fallback: ImageGenerationModelId,
): ImageGenerationModelId =>
  value === 'gemini-3.5-flash-image' ||
  value === 'imagen-4.0-generate-001' ||
  value === 'imagen-4.0-fast-generate-001' ||
  value === 'imagen-4.0-ultra-generate-001'
    ? value
    : fallback

const aspectRatioFromValue = (
  value: string,
  fallback: ImageGenerationAspectRatio,
): ImageGenerationAspectRatio =>
  value === '1:1' ||
  value === '3:4' ||
  value === '4:3' ||
  value === '9:16' ||
  value === '16:9'
    ? value
    : fallback

const imageSizeFromValue = (
  value: string,
  fallback: ImageGenerationImageSize,
): ImageGenerationImageSize =>
  value === '512' || value === '1K' || value === '2K' || value === '4K'
    ? value
    : fallback

const modelOptions = (
  provider: ImageGenerationProvider,
): ReadonlyArray<ImageGenerationModelId> =>
  provider === 'google-imagen' ? imagenModels : geminiModels

const actionText = (model: Model): string | undefined =>
  M.value(model.imageGeneration).pipe(
    M.tagsExhaustive({
      ImageGenerationIdle: () => undefined,
      ImageGenerationSubmitting: () => 'Generating...',
      ImageGenerationSucceeded: ({ images }) =>
        `${images.length} image${images.length === 1 ? '' : 's'} generated`,
      ImageGenerationFailed: ({ error }) => error,
    }),
  )

const optionView = (value: string, label: string): Html => {
  const h = html<Message>()

  return h.option([h.Value(value)], [label])
}

const fieldLabel = (label: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        'text-xs uppercase tracking-[0.18em] text-[#8f8f8f]',
      ),
    ],
    [label],
  )
}

const imageGrid = (images: ReadonlyArray<GeneratedImage>): Html => {
  const h = html<Message>()

  if (images.length === 0) {
    return h.div([], [])
  }

  return h.div(
    [Ui.className<Message>('grid grid-cols-1 gap-4 md:grid-cols-2')],
    images.map(image =>
      h.figure(
        [Ui.className<Message>('border border-[#1f1f1f] bg-[#050505]')],
        [
          h.img([
            Ui.className<Message>('aspect-square w-full object-cover'),
            h.Src(image.url),
            h.Alt(image.prompt),
          ]),
          h.figcaption(
            [
              Ui.className<Message>(
                'space-y-2 border-t border-[#1f1f1f] p-3 text-xs',
              ),
            ],
            [
              h.div([Ui.className<Message>('text-[#e8e8e8]')], [image.model]),
              h.div(
                [Ui.className<Message>('break-all text-[#8f8f8f]')],
                [image.key],
              ),
              h.a(
                [
                  Ui.className<Message>(
                    'inline-flex text-[#d7b94d] hover:text-[#f0d96f]',
                  ),
                  h.Href(image.url),
                  h.Target('_blank'),
                  h.Rel('noreferrer'),
                ],
                ['Open'],
              ),
            ],
          ),
        ],
      ),
    ),
  )
}

export const view = (model: Model): Html => {
  const h = html<Message>()
  const busy = model.imageGeneration._tag === 'ImageGenerationSubmitting'
  const message = actionText(model)
  const generatedImages =
    model.imageGeneration._tag === 'ImageGenerationSucceeded'
      ? model.imageGeneration.images
      : []

  return h.main(
    [
      Ui.className<Message>(
        'mx-auto flex w-full max-w-6xl flex-col gap-8 px-6 py-8',
      ),
    ],
    [
      h.header(
        [Ui.className<Message>('space-y-2')],
        [
          h.p(
            [
              Ui.className<Message>(
                'text-xs font-semibold uppercase tracking-[0.24em] text-[#8f8f8f]',
              ),
            ],
            ['Image generation'],
          ),
          h.h1(
            [Ui.className<Message>('text-2xl font-semibold text-[#f3f3f3]')],
            ['Generate images'],
          ),
        ],
      ),
      h.form(
        [
          Ui.className<Message>(
            'grid gap-4 border border-[#1f1f1f] bg-[#050505] p-4',
          ),
          h.OnSubmit(SubmittedImageGeneration()),
        ],
        [
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              fieldLabel('Prompt'),
              h.textarea(
                [
                  Ui.className<Message>(
                    'min-h-32 resize-y border border-[#2a2a2a] bg-black p-3 text-sm text-[#e8e8e8] outline-none focus:border-[#e8e8e8]',
                  ),
                  h.Value(model.imageGenerationPrompt),
                  h.OnInput(value => UpdatedImageGenerationPrompt({ value })),
                  h.Placeholder(
                    'A precise product screenshot, dark operational UI, table-first layout',
                  ),
                ],
                [],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-3 md:grid-cols-5')],
            [
              h.label(
                [Ui.className<Message>('grid gap-2')],
                [
                  fieldLabel('Provider'),
                  h.select(
                    [
                      Ui.className<Message>(controlClass),
                      h.Value(model.imageGenerationProvider),
                      h.OnInput(value =>
                        SelectedImageGenerationProvider({
                          provider: providerFromValue(value),
                        }),
                      ),
                    ],
                    providerOptions.map(option =>
                      optionView(option.value, option.label),
                    ),
                  ),
                ],
              ),
              h.label(
                [Ui.className<Message>('grid gap-2 md:col-span-2')],
                [
                  fieldLabel('Model'),
                  h.select(
                    [
                      Ui.className<Message>(controlClass),
                      h.Value(model.imageGenerationModel),
                      h.OnInput(value =>
                        SelectedImageGenerationModel({
                          model: modelFromValue(
                            value,
                            model.imageGenerationModel,
                          ),
                        }),
                      ),
                    ],
                    modelOptions(model.imageGenerationProvider).map(option =>
                      optionView(option, option),
                    ),
                  ),
                ],
              ),
              h.label(
                [Ui.className<Message>('grid gap-2')],
                [
                  fieldLabel('Ratio'),
                  h.select(
                    [
                      Ui.className<Message>(controlClass),
                      h.Value(model.imageGenerationAspectRatio),
                      h.OnInput(value =>
                        SelectedImageGenerationAspectRatio({
                          aspectRatio: aspectRatioFromValue(
                            value,
                            model.imageGenerationAspectRatio,
                          ),
                        }),
                      ),
                    ],
                    aspectRatios.map(option => optionView(option, option)),
                  ),
                ],
              ),
              h.label(
                [Ui.className<Message>('grid gap-2')],
                [
                  fieldLabel('Count'),
                  h.input([
                    Ui.className<Message>(controlClass),
                    h.Type('number'),
                    h.Min('1'),
                    h.Max('4'),
                    h.Value(String(model.imageGenerationCount)),
                    h.OnInput(value => UpdatedImageGenerationCount({ value })),
                  ]),
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-3 md:grid-cols-[160px_1fr]')],
            [
              h.label(
                [Ui.className<Message>('grid gap-2')],
                [
                  fieldLabel('Size'),
                  h.select(
                    [
                      Ui.className<Message>(controlClass),
                      h.Value(model.imageGenerationImageSize),
                      h.OnInput(value =>
                        SelectedImageGenerationImageSize({
                          imageSize: imageSizeFromValue(
                            value,
                            model.imageGenerationImageSize,
                          ),
                        }),
                      ),
                    ],
                    imageSizes.map(option => optionView(option, option)),
                  ),
                ],
              ),
              h.div(
                [Ui.className<Message>('flex items-end gap-3')],
                [
                  h.button(
                    [
                      Ui.className<Message>(
                        'h-10 border border-[#e8e8e8] bg-[#e8e8e8] px-4 text-sm font-semibold text-black disabled:cursor-not-allowed disabled:opacity-50',
                      ),
                      h.Type('submit'),
                      ...(busy ? [h.Disabled(true)] : []),
                    ],
                    [busy ? 'Generating' : 'Generate'],
                  ),
                  ...(message === undefined
                    ? []
                    : [
                        h.p(
                          [
                            Ui.className<Message>(
                              model.imageGeneration._tag ===
                                'ImageGenerationFailed'
                                ? 'text-sm text-[#ff8a8a]'
                                : 'text-sm text-[#8f8f8f]',
                            ),
                          ],
                          [message],
                        ),
                      ]),
                ],
              ),
            ],
          ),
        ],
      ),
      imageGrid(generatedImages),
    ],
  )
}
