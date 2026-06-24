import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// The real /login page. It is NOT its own isolated scene: it mounts as the
// OVERLAY of the shared persistent 3D pylon scene (the same keyed canvas the
// homepage `/` and `/tassadar` use), so navigating home <-> login is a
// continuous camera glide through ONE scene rather than a page cut. The scene
// canvas and the 75%-black readability scrim are supplied by the persistent
// shell (see persistentScene.ts); this view only renders the flush public
// header and the centered sign-in card floating over that scene.
//
// Login only authenticates; product access stays gated downstream (see auth
// audit). Sign in by email one-time code (the OpenAuth CodeProvider flow,
// started at /login/email) or GitHub (/login/github).

const overlayClass = 'absolute inset-0 z-10 flex flex-col overflow-y-auto'

const cardClass =
  'grid w-full max-w-[420px] gap-6 rounded-xl border border-white/10 bg-black/55 ' +
  'p-8 backdrop-blur-sm'

export const overlayView = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('persistent-scene-overlay', 'login'),
      h.DataAttribute('route', 'login'),
      Ui.className<Message>(overlayClass),
    ],
    [
      // Flush public header — same placement as the rest of the public chrome,
      // pinned to the top of the overlay with no dead band above it.
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Log in'),
          Ui.className<Message>(
            'flex min-h-0 flex-1 items-center justify-center px-4 py-12',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>(cardClass)],
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
