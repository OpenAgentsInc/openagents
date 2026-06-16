import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'

export const view = <Message>(workspaceId: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'workspace-invite'),
      Ui.className<Message>(
        'mx-auto grid min-h-[70svh] w-[min(100%,720px)] content-center gap-5 px-4 py-10 text-[#f1efe8]',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 border border-[#222] bg-[#010102] p-5 sm:p-6',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
              ),
            ],
            ['Workspace invite'],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 text-2xl font-medium tracking-normal text-[#f1efe8] sm:text-3xl',
              ),
            ],
            ['Open your project workspace'],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-base/7 text-white/65')],
            [
              'Your project setup is waiting. Sign in to review the seeded notes and starter workflows.',
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
            [workspaceId],
          ),
          h.a(
            [
              h.Href('/login/github'),
              Ui.className<Message>(
                'inline-flex min-h-10 w-fit items-center border border-[#f1efe8] bg-[#f1efe8] px-4 font-mono text-[0.8125rem] text-black hover:bg-white',
              ),
            ],
            ['Log in with GitHub'],
          ),
        ],
      ),
    ],
  )
}
