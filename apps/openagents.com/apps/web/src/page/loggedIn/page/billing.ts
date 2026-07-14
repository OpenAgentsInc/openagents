import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { Model } from '../model'

// Stable client-side tombstone for bookmarks while the server compatibility
// route returns the typed VP-1 410 envelope. There are deliberately no balance,
// package, checkout, coupon, card, or auto-top-up controls here.
export const view = (_model: Model): Html => {
  const h = html<Message>()

  return h.main(
    [
      Ui.className<Message>(
        'mx-auto flex min-h-[60vh] max-w-2xl flex-col justify-center px-6 py-16',
      ),
    ],
    [
      h.p(
        [
          Ui.className<Message>(
            'text-sm font-medium uppercase tracking-widest text-white/50',
          ),
        ],
        ['Retired'],
      ),
      h.h1(
        [Ui.className<Message>('mt-3 text-3xl font-semibold text-white')],
        ['Billing is no longer available'],
      ),
      h.p(
        [Ui.className<Message>('mt-4 text-base/7 text-white/70')],
        [
          'Payments, credits, wallets, payouts, settlement, and Sites are outside the Codex Workroom MVP. Formerly paid or credit-gated capacity is disabled; it has not become free capacity.',
        ],
      ),
    ],
  )
}
