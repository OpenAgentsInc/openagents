import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type {
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
} from '../route'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

type SiteCheckoutDemoRouteValue =
  SiteCheckoutDemoRoute | SiteCheckoutDemoReturnRoute

// Retained for source compatibility with old shell tests. It performs no fetch,
// checkout creation, polling, or payment action.
export const checkoutDemoScript = (
  _route: SiteCheckoutDemoRouteValue,
): string => 'document.documentElement.dataset.moneySurfaceRetired = "true";'

export const view = <Message>(
  _route: SiteCheckoutDemoRouteValue,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-h-dvh bg-black text-white')],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.DataAttribute('money-surface-retired', ''),
          Ui.className<Message>('mx-auto flex max-w-2xl flex-col px-6 py-24'),
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
            [Ui.className<Message>('mt-3 text-4xl font-semibold')],
            ['Sites checkout is no longer available'],
          ),
          h.p(
            [Ui.className<Message>('mt-4 text-base/7 text-white/70')],
            [
              'Sites, checkout, payments, credits, wallets, payouts, and settlement are outside the Codex Workroom MVP. No formerly paid capacity is available for free.',
            ],
          ),
        ],
      ),
    ],
  )
}

export const title = (_route: SiteCheckoutDemoRouteValue): string =>
  'Retired capability - OpenAgents'
