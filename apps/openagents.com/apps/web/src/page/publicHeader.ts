import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  blogRouter,
  chatRouter,
  docsRouter,
  forumRouter,
  homeRouter,
} from '../route'
import * as Ui from '../ui'

export type PublicHeaderAuthState<Message> =
  | {
      readonly _tag: 'LoggedOut'
    }
  | {
      readonly _tag: 'LoggedIn'
      readonly onLogout: Message
    }

type PublicNavItem = {
  readonly label: string
  readonly href: string
}

const navItems: ReadonlyArray<PublicNavItem> = [
  { label: 'Docs', href: docsRouter() },
  { label: 'Blog', href: blogRouter() },
  { label: 'Forum', href: forumRouter() },
]

const navLinkClass =
  'rounded px-2 py-1 text-base text-white/60 transition hover:bg-white/[0.04] hover:text-[#f1efe8] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] sm:text-sm'

const forumNavLinkClass =
  'rounded px-2 py-1 font-sans text-base text-white/85 transition hover:bg-white/10 hover:text-white focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white sm:text-sm'

export type PublicHeaderVariant = 'dark' | 'forum'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
  variant: PublicHeaderVariant = 'dark',
): Html => {
  const h = html<Message>()
  const isForum = variant === 'forum'
  const linkClass = isForum ? forumNavLinkClass : navLinkClass

  return h.header(
    [
      Ui.className<Message>(
        isForum
          ? 'border-b border-[#1f5a8c] bg-gradient-to-b from-[#5a9ad9] to-[#3a72b0] font-sans'
          : 'border-b border-[#222] bg-[#010102]',
      ),
    ],
    [
      h.nav(
        [
          h.AriaLabel('Global'),
          Ui.className<Message>(
            'mx-auto flex w-[min(100%,1120px)] flex-wrap items-center justify-between gap-3 px-4 py-3',
          ),
        ],
        [
          h.a(
            [
              h.Href(homeRouter()),
              h.AriaLabel('Homepage'),
              Ui.className<Message>(
                isForum
                  ? 'font-sans text-lg font-bold text-white'
                  : 'font-mono text-base font-medium text-[#f1efe8]',
              ),
            ],
            ['OpenAgents'],
          ),
          h.div(
            [Ui.className<Message>('hidden items-center gap-1 lg:flex')],
            Array.map(navItems, item =>
              h.a(
                [h.Href(item.href), Ui.className<Message>(linkClass)],
                [item.label],
              ),
            ),
          ),
          h.div(
            [Ui.className<Message>('flex items-center gap-2')],
            authState._tag === 'LoggedIn'
              ? [
                  h.a(
                    [h.Href(chatRouter()), Ui.className<Message>(linkClass)],
                    ['Workroom'],
                  ),
                  h.button(
                    [
                      h.Type('button'),
                      h.OnClick(authState.onLogout),
                      Ui.className<Message>(linkClass),
                    ],
                    ['Log out'],
                  ),
                ]
              : [
                  h.a(
                    [h.Href('/login/github'), Ui.className<Message>(linkClass)],
                    ['Log in'],
                  ),
                ],
          ),
          h.details(
            [Ui.className<Message>('w-full lg:hidden')],
            [
              h.summary(
                [
                  Ui.className<Message>(
                    'cursor-pointer list-none rounded py-2 text-base text-white/60 focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] [&::-webkit-details-marker]:hidden',
                  ),
                ],
                ['Menu'],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-1 border-t border-[#222] pt-3',
                  ),
                ],
                Array.map(navItems, item =>
                  h.a(
                    [h.Href(item.href), Ui.className<Message>(linkClass)],
                    [item.label],
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    ],
  )
}
