import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type { AutopilotOnboardingRoute } from '../route'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

const pageShellClass = 'min-h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const verticalLabel = (vertical: string | null): string =>
  vertical === null
    ? 'General workspace'
    : vertical
        .split('-')
        .filter(part => part.length > 0)
        .map(part => part[0]!.toUpperCase() + part.slice(1))
        .join(' ')

export const view = <Message>(
  route: AutopilotOnboardingRoute,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'autopilot-onboarding'),
      h.DataAttribute('autopilot-vertical', route.vertical ?? 'general'),
      Ui.className<Message>(pageShellClass),
    ],
    [
      PublicHeader.view(authState),
      h.main(
        [
          Ui.className<Message>(
            'mx-auto grid min-h-[calc(100dvh-4rem)] w-[min(100%,72rem)] content-center gap-8 px-4 py-14 sm:px-6 lg:px-8',
          ),
        ],
        [
          h.section(
            [Ui.className<Message>('max-w-3xl')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-xs font-semibold uppercase tracking-[0.08em] text-white/45',
                  ),
                ],
                ['OpenAgents Autopilot'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 mt-7 max-w-[13ch] text-balance text-5xl font-semibold leading-none text-white/90 sm:text-6xl lg:text-7xl',
                  ),
                ],
                ['Put Your Work On Autopilot'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 mt-7 max-w-[48ch] text-base leading-7 text-white/60 sm:text-lg sm:leading-8',
                  ),
                ],
                [
                  'Start with a narrow workspace, a real task, and reviewable evidence before anything expands.',
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'mt-8 inline-flex border border-white/15 bg-white/[0.04] px-3 py-2 text-sm text-white/70',
                  ),
                ],
                [verticalLabel(route.vertical)],
              ),
              h.div(
                [Ui.className<Message>('mt-9 flex flex-wrap gap-3')],
                [
                  h.a(
                    [
                      h.Href('/login/github'),
                      Ui.className<Message>(
                        'inline-grid min-h-11 place-items-center border border-[#f1efe8] bg-[#f1efe8] px-4 text-sm font-medium text-black no-underline hover:border-[#ffb400]',
                      ),
                    ],
                    ['Log in with GitHub'],
                  ),
                  h.a(
                    [
                      h.Href('/business'),
                      Ui.className<Message>(
                        'inline-grid min-h-11 place-items-center border border-white/20 px-4 text-sm font-medium text-white/75 no-underline hover:border-[#ffb400] hover:text-white',
                      ),
                    ],
                    ['See business options'],
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}
