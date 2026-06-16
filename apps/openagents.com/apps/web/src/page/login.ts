import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { constellationView } from '../scene/animations/constellation'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// The real /login page. Branded dark surface with the constellation network
// animation behind a centered card. Sign in by email one-time code (the OpenAuth
// CodeProvider flow, started at /login/email) or GitHub (/login/github). Login
// only authenticates; product access stays gated downstream (see auth audit).

const pageShellClass =
  'relative min-h-dvh overflow-hidden bg-[#000] text-[#f1efe8]'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 z-0 opacity-70',
          ),
        ],
        [constellationView<Message>()],
      ),
      h.div([Ui.className<Message>('relative z-10')], [PublicHeader.view(authState)]),
      h.main(
        [
          h.AriaLabel('Log in'),
          Ui.className<Message>(
            'relative z-10 grid min-h-[calc(100dvh-72px)] place-items-center px-4 py-16',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid w-full max-w-[420px] gap-6 rounded-xl border border-white/10 bg-black/55 p-8 backdrop-blur-sm',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('grid gap-2')],
                [
                  h.h1(
                    [
                      Ui.className<Message>(
                        'm-0 text-2xl font-medium tracking-tight text-[#f1efe8]',
                      ),
                    ],
                    ['Log in to OpenAgents'],
                  ),
                  h.p(
                    [Ui.className<Message>('m-0 text-sm/6 text-white/55')],
                    [
                      'Enter your email and we’ll send a one-time sign-in code, or continue with GitHub.',
                    ],
                  ),
                ],
              ),
              Ui.loginForm<Message>({
                emailPlaceholder: 'you@example.com',
                formAttrs: [h.Action('/login/email'), h.Method('get')],
                githubHref: '/login/github',
                submitLabel: 'Email me a code',
              }),
              h.p(
                [Ui.className<Message>('m-0 text-xs/5 text-white/40')],
                [
                  'Signing in creates your account. Access to workrooms and the operator console is granted to approved accounts.',
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}
