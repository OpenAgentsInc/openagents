import { clsx } from 'clsx'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  eyebrowClass,
  metaClass,
  statusDotClass,
  titleClass,
} from './primitives'
import type { Tone } from './primitives'
import {
  stylexAttrs,
  stylexFallback,
} from './stylex-foldkit'

const emptyStateStyles = {
  root: stylexFallback('oa-ui-empty-state-root'),
  body: stylexFallback('oa-ui-empty-state-body'),
}

export const alert = <Message>(input: {
  title: string
  body?: string
  tone?: Tone
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        clsx(
          'grid grid-cols-[auto_minmax(0,1fr)] gap-3 border bg-[#010102] p-3',
          {
            'border-[#222]':
              input.tone === undefined || input.tone === 'neutral',
            'border-[#ffb400]': input.tone === 'accent',
            'border-[#00c853]': input.tone === 'positive',
            'border-[#ff6f00]': input.tone === 'warning',
            'border-[#d32f2f]': input.tone === 'negative',
            'border-[#2979ff]': input.tone === 'info',
          },
        ),
      ),
    ],
    [
      h.span([h.Class(statusDotClass(input.tone ?? 'neutral'))], []),
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class(titleClass)], [input.title]),
          input.body === undefined
            ? null
            : h.p([h.Class(metaClass)], [input.body]),
        ],
      ),
    ],
  )
}

export const emptyState = <Message>(input: {
  title: string
  body?: string
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.div(
    stylexAttrs<Message>(emptyStateStyles.root),
    [
      h.div(
        stylexAttrs<Message>(emptyStateStyles.body),
        [
          h.p([h.Class(eyebrowClass)], ['Empty']),
          h.h3([h.Class(titleClass)], [input.title]),
          input.body === undefined
            ? null
            : h.p([h.Class(metaClass)], [input.body]),
        ],
      ),
      input.action ?? null,
    ],
  )
}
