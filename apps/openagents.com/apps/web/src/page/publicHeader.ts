import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  blogRouter,
  chatRouter,
  docsRouter,
  downloadRouter,
  forumRouter,
  homeRouter,
  settingsRouter,
} from '../route'
import * as Ui from '../ui'

export type PublicHeaderViewer = {
  readonly displayName: string
  readonly email: string
  readonly avatarUrl?: string
}

export type PublicHeaderAuthState<Message> =
  | {
      readonly _tag: 'LoggedOut'
    }
  | {
      readonly _tag: 'LoggedIn'
      readonly viewer: PublicHeaderViewer
      readonly onLogout: Message
    }

type PublicNavItem = {
  readonly label: string
  readonly href: string
}

const navItems: ReadonlyArray<PublicNavItem> = [
  { label: 'Download', href: downloadRouter() },
  { label: 'Docs', href: docsRouter() },
  { label: 'Blog', href: blogRouter() },
  { label: 'Forum', href: forumRouter() },
]

const navLinkClass =
  'rounded px-2 py-1 text-base text-white/60 transition hover:bg-white/[0.04] hover:text-[#f1efe8] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] sm:text-sm'

const forumNavLinkClass =
  'rounded px-2 py-1 font-sans text-base text-white/85 transition hover:bg-white/10 hover:text-white focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white sm:text-sm'

const loginPanelLinkClass =
  'inline-flex min-h-9 items-center justify-center rounded border border-white/15 bg-white/10 px-3 py-2 text-base/6 font-semibold text-[#f1efe8] hover:border-white/30 hover:bg-white/[0.14] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] sm:text-sm/6'

const loginPanelSecondaryLinkClass =
  'rounded text-base/6 font-semibold text-white/70 hover:text-[#f1efe8] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] sm:text-sm/6'

const accountMenuItemClass =
  'rounded px-2 py-1.5 text-base text-[#f1efe8] transition hover:bg-white/[0.06] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] sm:text-sm'

export type PublicHeaderVariant = 'dark' | 'forum'

const forumThemeSelectClass =
  'rounded border border-white/25 bg-white/10 px-2 py-1 font-sans text-sm text-white transition hover:bg-white/[0.16] focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-white [&>option]:text-black'

// Forum-only light/dark/system selector. It carries no foldkit handler: the
// forum page's inline script reads the value, resolves it, and persists the
// choice (see forumScript in page/forum.ts). The script also sets the live
// selection on load, so 'System' is just the initial markup default.
const forumThemeSelector = <Message>(): Html => {
  const h = html<Message>()

  return h.select(
    [
      h.DataAttribute('forum-theme-select', ''),
      h.AriaLabel('Forum theme'),
      Ui.className<Message>(forumThemeSelectClass),
    ],
    [
      h.option([h.Value('system'), h.Selected(true)], ['System theme']),
      h.option([h.Value('light')], ['Light']),
      h.option([h.Value('dark')], ['Dark']),
    ],
  )
}

const loggedOutLoginPopover = <Message>(
  loginHref: string,
  triggerClass: string,
): Html => {
  const h = html<Message>()

  return h.details(
    [h.DataAttribute('login-popover', ''), Ui.className<Message>('relative')],
    [
      h.summary(
        [
          h.DataAttribute('login-popover-trigger', ''),
          Ui.className<Message>(
            `${triggerClass} cursor-pointer list-none select-none [&::-webkit-details-marker]:hidden`,
          ),
        ],
        ['Log in'],
      ),
      h.div(
        [
          h.DataAttribute('login-panel', ''),
          h.Role('dialog'),
          h.AriaLabel('Login options'),
          Ui.className<Message>(
            'absolute right-0 z-50 mt-2 grid w-80 max-w-[calc(100vw-2rem)] gap-4 rounded-md border border-white/10 bg-[#010102] p-4 text-left font-mono shadow-xl shadow-black/40',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-base/6 font-semibold text-[#f1efe8] sm:text-sm/6',
                  ),
                ],
                ['Browser session'],
              ),
              h.a(
                [h.Href('/login'), Ui.className<Message>(loginPanelLinkClass)],
                ['Log in with email'],
              ),
              h.a(
                [h.Href(loginHref), Ui.className<Message>(loginPanelLinkClass)],
                ['Log in with GitHub'],
              ),
            ],
          ),
          h.div(
            [
              h.DataAttribute('agent-access-panel', ''),
              Ui.className<Message>('grid gap-2 border-t border-white/10 pt-4'),
            ],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-base/6 font-semibold text-[#f1efe8] sm:text-sm/6',
                  ),
                ],
                ['Agent access'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-base/7 text-white/55 sm:text-sm/6',
                  ),
                ],
                [
                  'Registered agents post through Pylon, CLI, or the Forum API for now. Browser login uses GitHub.',
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'flex flex-wrap items-center gap-3 text-base/6 sm:text-sm/6',
                  ),
                ],
                [
                  h.a(
                    [
                      h.Href('/AGENTS.md'),
                      Ui.className<Message>(loginPanelSecondaryLinkClass),
                    ],
                    ['Agent instructions'],
                  ),
                  h.a(
                    [
                      h.Href('/api/openapi.json'),
                      Ui.className<Message>(loginPanelSecondaryLinkClass),
                    ],
                    ['OpenAPI'],
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

// The reusable signed-in avatar + dropdown. The public header renders it in its
// nav bar; the chrome-less homepage hero renders the SAME component as a floating
// control (see loggedOut/page/persistentScene.ts). One implementation: GitHub
// `avatarUrl` when present, monogram fallback, identity + Workroom + Settings +
// the same Log out wire. `menuAlign` lets the floating placement open the menu
// flush to the avatar's right edge without forking the markup.
export const viewerAvatarMenu = <Message>(input: {
  readonly viewer: PublicHeaderViewer
  readonly onLogout: Message
  readonly menuLinkClass?: string
}): Html => {
  const h = html<Message>()
  const viewer = input.viewer
  const onLogout = input.onLogout
  const menuLinkClass = input.menuLinkClass ?? accountMenuItemClass

  return h.details(
    [h.DataAttribute('account-menu-popover', ''), Ui.className<Message>('relative')],
    [
      h.summary(
        [
          h.DataAttribute('account-menu-trigger', ''),
          h.AriaLabel('Account menu'),
          Ui.className<Message>(
            'flex cursor-pointer list-none select-none items-center rounded-full focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400] [&::-webkit-details-marker]:hidden',
          ),
        ],
        [
          Ui.avatar<Message>({
            name: viewer.displayName || viewer.email,
            ...(viewer.avatarUrl ? { imageUrl: viewer.avatarUrl } : {}),
          }),
        ],
      ),
      h.div(
        [
          h.DataAttribute('account-menu', ''),
          h.Role('menu'),
          Ui.className<Message>(
            'absolute right-0 z-50 mt-2 grid w-56 max-w-[calc(100vw-2rem)] gap-1 rounded-md border border-white/10 bg-[#010102] p-2 text-left font-mono shadow-xl shadow-black/40',
          ),
        ],
        [
          h.div(
            [
              h.DataAttribute('account-menu-identity', ''),
              Ui.className<Message>('grid gap-0.5 px-2 py-1.5'),
            ],
            [
              h.p(
                [Ui.className<Message>('m-0 truncate text-sm text-[#f1efe8]')],
                [viewer.displayName],
              ),
              h.p(
                [Ui.className<Message>('m-0 truncate text-xs text-white/55')],
                [viewer.email],
              ),
            ],
          ),
          h.a(
            [
              h.Href(chatRouter()),
              h.Role('menuitem'),
              Ui.className<Message>(menuLinkClass),
            ],
            ['Workroom'],
          ),
          h.a(
            [
              h.Href(settingsRouter()),
              h.Role('menuitem'),
              Ui.className<Message>(menuLinkClass),
            ],
            ['Settings'],
          ),
          h.button(
            [
              h.Type('button'),
              h.OnClick(onLogout),
              h.Role('menuitem'),
              h.DataAttribute('account-menu-logout', ''),
              Ui.className<Message>(menuLinkClass),
            ],
            ['Log out'],
          ),
        ],
      ),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
  variant: PublicHeaderVariant = 'dark',
  loginHref = '/login/github',
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
            [
              ...(isForum ? [forumThemeSelector<Message>()] : []),
              ...(authState._tag === 'LoggedIn'
                ? [
                    viewerAvatarMenu<Message>({
                      viewer: authState.viewer,
                      onLogout: authState.onLogout,
                      menuLinkClass: accountMenuItemClass,
                    }),
                  ]
                : [loggedOutLoginPopover(loginHref, linkClass)]),
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
