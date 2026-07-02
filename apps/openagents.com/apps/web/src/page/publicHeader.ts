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
  'khala-focus rounded px-2 py-1 font-mono text-base text-[var(--oa-color-khala-text-muted)] transition-colors hover:bg-[var(--oa-color-khala-surface-active)] hover:text-[var(--oa-color-khala-energy-cyan)] sm:text-sm'

const loginPanelLinkClass =
  'khala-focus inline-flex min-h-9 items-center justify-center rounded border border-[var(--oa-color-khala-border-strong)] bg-[var(--oa-color-khala-surface-active)] px-3 py-2 text-base/6 font-semibold text-[var(--oa-color-khala-energy-button-text)] transition-colors hover:border-[var(--oa-color-khala-energy-cyan)] hover:bg-[var(--oa-color-khala-surface-muted)] hover:text-[var(--oa-color-khala-energy-text-strong)] sm:text-sm/6'

const loginPanelSecondaryLinkClass =
  'khala-focus rounded text-base/6 font-semibold text-[var(--oa-color-khala-energy-soft)] transition-colors hover:text-[var(--oa-color-khala-energy-cyan)] sm:text-sm/6'

const accountMenuItemClass =
  'khala-focus rounded px-2 py-1.5 text-base text-[var(--oa-color-khala-text-primary)] transition-colors hover:bg-[var(--oa-color-khala-surface-active)] hover:text-[var(--oa-color-khala-energy-cyan)] sm:text-sm'

export type PublicHeaderVariant = 'dark' | 'forum'

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
            'khala-panel absolute right-0 z-50 mt-2 grid w-80 max-w-[calc(100vw-2rem)] gap-4 rounded-md border border-[var(--oa-color-khala-border)] bg-[var(--oa-color-khala-surface-raised)] p-4 text-left font-mono',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-base/6 font-semibold text-[var(--oa-color-khala-text-bright)] sm:text-sm/6',
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
              Ui.className<Message>(
                'grid gap-2 border-t border-[var(--oa-color-khala-border)] pt-4',
              ),
            ],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-base/6 font-semibold text-[var(--oa-color-khala-text-bright)] sm:text-sm/6',
                  ),
                ],
                ['Agent access'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-base/7 text-[var(--oa-color-khala-text-muted)] sm:text-sm/6',
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
    [
      h.DataAttribute('account-menu-popover', ''),
      Ui.className<Message>('relative'),
    ],
    [
      h.summary(
        [
          h.DataAttribute('account-menu-trigger', ''),
          h.AriaLabel('Account menu'),
          Ui.className<Message>(
            'khala-focus flex cursor-pointer list-none select-none items-center rounded-full [&::-webkit-details-marker]:hidden',
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
            'khala-panel absolute right-0 z-50 mt-2 grid w-56 max-w-[calc(100vw-2rem)] gap-1 rounded-md border border-[var(--oa-color-khala-border)] bg-[var(--oa-color-khala-surface-raised)] p-2 text-left font-mono',
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
                [
                  Ui.className<Message>(
                    'm-0 truncate text-sm text-[var(--oa-color-khala-text-bright)]',
                  ),
                ],
                [viewer.displayName],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 truncate text-xs text-[var(--oa-color-khala-text-muted)]',
                  ),
                ],
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
  const linkClass = navLinkClass

  return h.header(
    [
      h.DataAttribute('public-header-variant', variant),
      Ui.className<Message>(
        'border-b border-[var(--oa-color-khala-border)] bg-[var(--oa-color-khala-surface)] font-mono text-[var(--oa-color-khala-text-primary)]',
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
                'khala-focus rounded font-mono text-base font-medium text-[var(--oa-color-khala-text-bright)] transition-colors hover:text-[var(--oa-color-khala-energy-cyan)]',
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
                    'khala-focus cursor-pointer list-none rounded py-2 text-base text-[var(--oa-color-khala-text-muted)] transition-colors hover:text-[var(--oa-color-khala-energy-cyan)] [&::-webkit-details-marker]:hidden',
                  ),
                ],
                ['Menu'],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-1 border-t border-[var(--oa-color-khala-border)] pt-3',
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
