import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { orderRouter } from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { Model } from '../model'

export const view = (_model: Model): Html => {
  const h = html<Message>()

  return h.section(
    [
      Ui.className<Message>(
        'flex min-h-full items-center justify-center px-4 py-10',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('grid w-full max-w-sm gap-5')],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Access']),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-lg font-semibold leading-none text-white',
                  ),
                ],
                ['Open beta is live'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-sm/6 text-white/55')],
                [
                  'Start a free public software order with your GitHub account.',
                ],
              ),
            ],
          ),
          Ui.linkButton<Message>({
            href: orderRouter(),
            label: 'Continue to order',
            size: 'md',
            variant: 'secondary',
          }),
        ],
      ),
    ],
  )
}
