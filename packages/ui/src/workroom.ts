import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { type IconName, iconView } from './icon'
import { codeBlock, keyValueRows, mediaObject } from './data-display'
import { tabBar } from './navigation'
import {
  kitFamily,
  metaClass,
  motionOdometerClass,
  motionPaneOpenClass,
  motionRowHoverClass,
  motionStatusMorphAnimationClass,
  motionStatusMorphClass,
  motionTextRevealAnimationClass,
  rowClass,
  statusDotClass,
  surfaceActiveClass,
  titleClass,
} from './primitives'
import type {
  DescriptionItem,
  KeyValueItem,
  NavItem,
  Tone,
  WorkroomAccountMenuItem,
  WorkroomChecklistItem,
  WorkroomFileItem,
  WorkroomSessionItem,
  WorkroomSidebarNavSection,
  WorkroomSidebarSessionSection,
  WorkroomTab,
  WorkroomTimelineMessage,
  WorkroomTimelinePart,
} from './primitives'
import { avatar, buttonGroup, linkButton } from './shared'
import { stylexAttrs, stylexAttrsWithClass } from './stylex-foldkit'
import { v4ChatMessage } from './v4'
import { workroomStyles } from './workroom-styles'

export const workroomShell = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      h.Class(
        clsx(
          motionPaneOpenClass,
          'isolate grid h-dvh w-screen grid-cols-[280px_minmax(0,1fr)] overflow-hidden bg-[#000] font-mono text-[#f1efe8] antialiased max-[1100px]:grid-cols-[232px_minmax(0,1fr)] max-[760px]:grid-cols-[minmax(0,1fr)]',
        ),
      ),
    ],
    children,
  )
}

export const workroomRail = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.aside(
    [
      ...attrs,
      h.Class(
        clsx(
          motionPaneOpenClass,
          'flex min-h-0 min-w-0 flex-col border-r border-[#222] bg-[#010101] max-[760px]:hidden',
        ),
      ),
    ],
    children,
  )
}

const workroomSessionTone = (status: WorkroomSessionItem['status']): Tone => {
  if (status === 'active') {
    return 'accent'
  }

  if (status === 'complete') {
    return 'positive'
  }

  if (status === 'failed') {
    return 'negative'
  }

  return 'neutral'
}

export const workroomSessionRail = <Message>(input: {
  brand: string
  title: string
  userName: string
  userEmail: string
  sessions: ReadonlyArray<WorkroomSessionItem>
  footerRows?: ReadonlyArray<DescriptionItem>
  action?: Html
  settingsHref?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return workroomRail<Message>(
    [
      h.div(
        [
          kitFamily<Message>('headings/card-headings'),
          h.Class(
            'flex min-h-16 items-center justify-between gap-3 border-b border-[#222] px-3.5',
          ),
        ],
        [
          mediaObject<Message>({
            title: input.title,
            body: input.brand,
          }),
          input.action ?? null,
        ],
      ),
      mediaObject<Message>({
        title: input.userName,
        body: input.userEmail,
      }),
      h.ul(
        [
          kitFamily<Message>('lists/stacked-lists'),
          h.Role('list'),
          h.Class(
            'm-0 flex min-h-0 flex-1 list-none flex-col gap-0.5 overflow-auto overscroll-contain p-2',
          ),
        ],
        input.sessions.map(item =>
          h.li(
            [],
            [
              h.a(
                [
                  h.Href(item.href ?? '#'),
                  h.Class(
                    clsx(
                      rowClass,
                      'border border-transparent p-[9px_8px] text-inherit no-underline hover:border-[#333] hover:bg-[#080808]',
                      {
                        'border-[#333] bg-[#080808]': item.active === true,
                      },
                    ),
                  ),
                ],
                [
                  h.span(
                    [h.Class(statusDotClass(workroomSessionTone(item.status)))],
                    [],
                  ),
                  h.span(
                    [h.Class('flex min-w-0 flex-col gap-0.5')],
                    [
                      h.span([h.Class(titleClass)], [item.title]),
                      h.span([h.Class(metaClass)], [item.detail]),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
      h.div(
        [h.Class('grid gap-2 border-t border-[#222] px-3.5 pb-3.5 pt-3')],
        [
          ...(input.footerRows ?? []).map(row =>
            h.div(
              [
                h.Class(
                  'flex justify-between gap-3 text-[0.75rem] text-white/60',
                ),
              ],
              [
                h.span([], [row.label]),
                h.span(
                  [h.Class(motionOdometerClass), h.Key(row.value)],
                  [row.value],
                ),
              ],
            ),
          ),
          input.settingsHref === undefined
            ? null
            : h.a(
                [
                  h.Href(input.settingsHref),
                  h.Class('text-[#f1efe8] underline underline-offset-[3px]'),
                ],
                ['Settings'],
              ),
        ],
      ),
    ],
    [kitFamily<Message>('application-shells/sidebar'), ...(input.attrs ?? [])],
  )
}

const workroomSidebarNavRow = <Message>(
  item: NavItem,
  density: 'default' | 'compact' = 'default',
): Html => {
  const h = html<Message>()

  return h.li(
    [h.Class('min-w-0')],
    [
      h.a(
        [
          h.Href(item.href),
          h.Class(
            clsx(
              'grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent text-[0.75rem] text-white/55 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
              motionRowHoverClass,
              density === 'compact' ? 'min-h-6 px-2' : 'min-h-8 px-2.5',
              {
                'border-[#333] bg-[#080808] text-[#f1efe8]':
                  item.active === true,
              },
            ),
          ),
        ],
        [
          h.span([h.Class('truncate')], [item.label]),
          item.meta === undefined
            ? null
            : h.span(
                [h.Class('min-w-0 truncate text-[0.6875rem] text-white/35')],
                [item.meta],
              ),
        ],
      ),
    ],
  )
}

const workroomSidebarSessionRow = <Message>(
  item: WorkroomSessionItem,
  density: 'default' | 'compact' = 'default',
): Html => {
  const h = html<Message>()
  const hasDetail = item.detail.trim() !== ''

  return h.li(
    [h.Class('min-w-0')],
    [
      h.a(
        [
          h.Href(item.href ?? '#'),
          h.Class(
            clsx(
              'grid w-full min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2.5 border border-transparent text-inherit no-underline hover:border-[#333] hover:bg-[#080808]',
              motionRowHoverClass,
              density === 'compact' ? 'min-h-6 px-2 py-0' : 'min-h-8 px-2 py-1',
              {
                'border-[#ffb400]/70 bg-[#080808] text-[#f1efe8]':
                  item.active === true,
              },
            ),
          ),
        ],
        [
          h.span(
            [h.Class(statusDotClass(workroomSessionTone(item.status)))],
            [],
          ),
          h.span(
            [h.Class('flex min-w-0 items-baseline gap-2 overflow-hidden')],
            [
              h.span(
                [
                  h.Class(
                    'min-w-0 shrink overflow-hidden text-ellipsis whitespace-nowrap text-[0.75rem] font-medium text-white/85',
                  ),
                ],
                [item.title],
              ),
              hasDetail
                ? h.span(
                    [
                      h.Class(
                        'min-w-0 shrink-[2] overflow-hidden text-ellipsis whitespace-nowrap text-[0.6875rem] text-white/35',
                      ),
                    ],
                    [item.detail],
                  )
                : null,
            ],
          ),
          item.attention === true
            ? h.span(
                [
                  h.Class(
                    'border border-[#ffb400]/70 px-1.5 py-[3px] text-[0.625rem] uppercase leading-none tracking-[0.08em] text-[#ffb400]',
                  ),
                ],
                ['attn'],
              )
            : null,
        ],
      ),
    ],
  )
}

const workroomSidebarSection = <Message>(input: {
  title: string
  children: ReadonlyArray<Html | string>
}): Html => {
  const h = html<Message>()

  return h.details(
    [h.Attribute('open', ''), h.Class('group grid min-w-0 gap-1')],
    [
      h.summary(
        [
          h.Class(
            'grid min-w-0 cursor-pointer list-none grid-cols-[minmax(0,1fr)_auto] items-center px-2 py-1 text-[0.6875rem] font-semibold uppercase tracking-[0.06em] text-white/40 hover:text-white/65 [&::-webkit-details-marker]:hidden',
          ),
        ],
        [
          h.span([h.Class('truncate')], [input.title]),
          iconView<Message>(
            'ChevronRight',
            'size-4 text-white/30 transition-transform group-open:rotate-90',
          ),
        ],
      ),
      ...input.children,
    ],
  )
}

export const workroomSidebar = <Message>(input: {
  product: string
  productAttrs?: ReadonlyArray<Attribute<Message>>
  workspace: string
  userName: string
  userEmail: string
  userAvatarUrl?: string
  navSections: ReadonlyArray<WorkroomSidebarNavSection>
  sessionSections: ReadonlyArray<WorkroomSidebarSessionSection>
  accountMenuItems?: ReadonlyArray<WorkroomAccountMenuItem<Message>>
  footerRows?: ReadonlyArray<DescriptionItem>
  headerActions?: ReadonlyArray<Html | string>
  navDensity?: 'default' | 'compact'
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const navDensity = input.navDensity ?? 'default'
  const accountMenuItems = input.accountMenuItems ?? []
  const accountButtonClass =
    'grid w-full cursor-pointer select-none list-none grid-cols-[auto_minmax(0,1fr)] items-center gap-2 border border-transparent px-2 py-1.5 text-left hover:border-[#333] hover:bg-[#080808] [&::-webkit-details-marker]:hidden'
  const accountMenuItemClass = (
    tone: WorkroomAccountMenuItem<Message>['tone'],
  ): string =>
    clsx(
      'block w-full cursor-pointer select-none border border-transparent bg-transparent px-2 py-1.5 text-left font-[inherit] text-[0.75rem] no-underline hover:border-[#333] hover:bg-[#080808]',
      tone === 'danger'
        ? 'text-[#ff6f00] hover:text-[#ffb400]'
        : 'text-white/60 hover:text-[#f1efe8]',
    )

  return workroomRail<Message>(
    [
      h.div(
        [
          kitFamily<Message>('headings/card-headings'),
          h.Class('flex h-12 items-center border-b border-[#222] px-3'),
        ],
        [
          h.div(
            [h.Class('flex w-full items-center justify-between gap-3')],
            [
              h.a(
                [
                  h.Href('/'),
                  h.Class(
                    'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-[#f1efe8] no-underline hover:text-white',
                  ),
                  ...(input.productAttrs ?? []),
                ],
                [input.product],
              ),
            ],
          ),
        ],
      ),
      h.nav(
        [
          h.AriaLabel(`${input.product} navigation`),
          h.Class(
            'flex min-h-0 flex-1 flex-col gap-4 overflow-auto overscroll-contain p-2',
          ),
        ],
        [
          input.headerActions === undefined
            ? null
            : h.div([h.Class('grid gap-1 pb-1')], input.headerActions),
          ...input.sessionSections.map(section =>
            workroomSidebarSection<Message>({
              title: section.title,
              children: [
                h.ul(
                  [
                    h.Role('list'),
                    h.Class(
                      clsx(
                        'm-0 grid min-w-0 list-none p-0',
                        navDensity === 'compact' ? 'gap-0' : 'gap-0.5',
                      ),
                    ),
                  ],
                  section.items.map(item =>
                    workroomSidebarSessionRow<Message>(item, navDensity),
                  ),
                ),
              ],
            }),
          ),
          ...input.navSections.map(section =>
            workroomSidebarSection<Message>({
              title: section.title,
              children: [
                h.ul(
                  [
                    h.Role('list'),
                    h.Class(
                      clsx(
                        'm-0 grid min-w-0 list-none p-0',
                        navDensity === 'compact' ? 'gap-0' : 'gap-0.5',
                      ),
                    ),
                  ],
                  section.items.map(item =>
                    workroomSidebarNavRow<Message>(item, navDensity),
                  ),
                ),
              ],
            }),
          ),
        ],
      ),
      h.div(
        [h.Class('grid gap-2 border-t border-[#222] px-2 pb-2 pt-2')],
        [
          ...(input.footerRows ?? []).map(row =>
            h.div(
              [
                h.Class(
                  'flex justify-between gap-3 text-[0.75rem] text-white/45',
                ),
              ],
              [
                h.span([], [row.label]),
                h.span(
                  [h.Class(motionOdometerClass), h.Key(row.value)],
                  [row.value],
                ),
              ],
            ),
          ),
          h.div(
            [h.Class('relative')],
            [
              h.details(
                [
                  h.DataAttribute('component', 'account-menu'),
                  h.Class('group relative'),
                ],
                [
                  h.summary(
                    [h.Class(accountButtonClass)],
                    [
                      avatar<Message>({
                        name: input.userName,
                        ...(input.userAvatarUrl === undefined
                          ? {}
                          : { imageUrl: input.userAvatarUrl }),
                        size: 'sm',
                      }),
                      h.span(
                        [h.Class('grid min-w-0 gap-0.5')],
                        [
                          h.span([h.Class(titleClass)], [input.userName]),
                          h.span([h.Class(metaClass)], [input.userEmail]),
                        ],
                      ),
                    ],
                  ),
                  accountMenuItems.length === 0
                    ? null
                    : h.div(
                        [
                          h.Class(
                            'absolute bottom-[calc(100%+0.5rem)] left-0 right-0 z-20 hidden border border-[#333] bg-[#050505] p-1 shadow-[0_12px_40px_rgba(0,0,0,0.55)] group-open:grid',
                          ),
                        ],
                        accountMenuItems.map(item =>
                          item.href === undefined
                            ? h.button(
                                [
                                  ...(item.attrs ?? []),
                                  h.Type('button'),
                                  h.Class(accountMenuItemClass(item.tone)),
                                ],
                                [item.label],
                              )
                            : h.a(
                                [
                                  ...(item.attrs ?? []),
                                  h.Href(item.href),
                                  h.Class(accountMenuItemClass(item.tone)),
                                ],
                                [item.label],
                              ),
                        ),
                      ),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
    [kitFamily<Message>('application-shells/sidebar'), ...(input.attrs ?? [])],
  )
}

export const workroomSidebarActionLink = <Message>(input: {
  href: string
  label: string
  icon?: IconName
}): Html => {
  const h = html<Message>()

  if (input.icon !== undefined) {
    return h.a(
      [
        h.Href(input.href),
        h.AriaLabel(input.label),
        h.Class(
          clsx(
            motionRowHoverClass,
            'grid min-h-8 w-full cursor-pointer grid-cols-[1rem_minmax(0,1fr)] items-center gap-2 border border-transparent bg-transparent px-2 text-left font-[inherit] text-[0.8125rem] font-medium text-white/75 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
          ),
        ),
      ],
      [
        iconView<Message>(input.icon, 'size-4 text-white/55'),
        h.span([h.Class('truncate')], [input.label]),
      ],
    )
  }

  return h.a(
    [
      h.Href(input.href),
      h.Class(
        clsx(
          motionRowHoverClass,
          'border border-[#333] px-2 py-[5px] text-[0.6875rem] text-white/55 no-underline hover:bg-[#080808] hover:text-[#f1efe8]',
        ),
      ),
    ],
    [input.label],
  )
}

export const workroomSidebarActionButton = <Message>(input: {
  label: string
  icon: IconName
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      h.Type('button'),
      h.AriaLabel(input.label),
      h.Class(
        clsx(
          motionRowHoverClass,
          'grid min-h-8 w-full cursor-pointer grid-cols-[1rem_minmax(0,1fr)] items-center gap-2 border border-transparent bg-transparent px-2 text-left font-[inherit] text-[0.8125rem] font-medium text-white/75 hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
        ),
      ),
    ],
    [
      iconView<Message>(input.icon, 'size-4 text-white/55'),
      h.span([h.Class('truncate')], [input.label]),
    ],
  )
}

const workroomMobileNavRow = <Message>(item: NavItem): Html => {
  const h = html<Message>()

  return h.li(
    [],
    [
      h.a(
        [
          h.Href(item.href),
          h.Class(
            clsx(
              'block border border-[#222] bg-[#020202] px-3 py-2 text-sm text-white/65 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8] sm:text-[0.8125rem]',
              motionRowHoverClass,
              {
                'border-[#333] bg-[#080808] text-[#f1efe8]':
                  item.active === true,
              },
            ),
          ),
        ],
        [item.label],
      ),
    ],
  )
}

const workroomMobileSessionRow = <Message>(item: WorkroomSessionItem): Html => {
  const h = html<Message>()
  const hasDetail = item.detail.trim() !== ''

  return h.li(
    [],
    [
      h.a(
        [
          h.Href(item.href ?? '#'),
          h.Class(
            clsx(
              'grid border border-[#222] bg-[#020202] px-3 py-2 text-white/65 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
              motionRowHoverClass,
              {
                'border-[#333] bg-[#080808] text-[#f1efe8]':
                  item.active === true,
              },
            ),
          ),
        ],
        [
          h.span(
            [h.Class('flex min-w-0 items-baseline gap-2')],
            [
              h.span(
                [h.Class('min-w-0 truncate text-sm sm:text-[0.8125rem]')],
                [item.title],
              ),
              hasDetail
                ? h.span(
                    [h.Class('min-w-0 truncate text-[0.75rem] text-white/35')],
                    [item.detail],
                  )
                : null,
            ],
          ),
        ],
      ),
    ],
  )
}

export const workroomMobileSidebar = <Message>(input: {
  product: string
  userName: string
  navSections: ReadonlyArray<WorkroomSidebarNavSection>
  sessionSections: ReadonlyArray<WorkroomSidebarSessionSection>
  headerActions?: ReadonlyArray<Html | string>
}): Html => {
  const h = html<Message>()

  return h.details(
    [h.Class('hidden border-b border-[#222] bg-[#010102] max-[760px]:block')],
    [
      h.summary(
        [
          h.Class(
            'flex min-h-12 cursor-pointer items-center justify-between gap-3 px-3 text-[0.8125rem] text-[#f1efe8]',
          ),
        ],
        [
          h.span([], [input.product]),
          h.span([h.Class('text-white/45')], [input.userName]),
        ],
      ),
      h.nav(
        [
          h.AriaLabel(`${input.product} mobile navigation`),
          h.Class('grid gap-4 border-t border-[#222] p-3'),
        ],
        [
          input.headerActions === undefined
            ? null
            : h.div([h.Class('grid gap-1')], input.headerActions),
          ...input.sessionSections.map(section =>
            workroomSidebarSection<Message>({
              title: section.title,
              children: [
                h.ul(
                  [h.Role('list'), h.Class('m-0 grid list-none gap-1 p-0')],
                  section.items.map(workroomMobileSessionRow<Message>),
                ),
              ],
            }),
          ),
          ...input.navSections.map(section =>
            workroomSidebarSection<Message>({
              title: section.title,
              children: [
                h.ul(
                  [h.Role('list'), h.Class('m-0 grid list-none gap-1 p-0')],
                  section.items.map(workroomMobileNavRow<Message>),
                ),
              ],
            }),
          ),
        ],
      ),
    ],
  )
}

export const workroomRouteMain = <Message>(input: {
  key: string
  variant: 'chat' | 'scroll'
  mobileSidebar: Html
  children: ReadonlyArray<Html | string>
}): Html => {
  const h = html<Message>()

  return h.keyed('main')(
    input.key,
    [
      h.Class(
        input.variant === 'chat'
          ? clsx(
              motionPaneOpenClass,
              'flex min-h-0 min-w-0 flex-col overflow-hidden',
            )
          : clsx(motionPaneOpenClass, 'min-h-0 min-w-0 overflow-auto'),
      ),
    ],
    [input.mobileSidebar, ...input.children],
  )
}

export const workroomChatRoute = <Message>(child: Html): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...stylexAttrsWithClass<Message>(
        clsx(
          motionPaneOpenClass,
          'flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden',
        ),
        workroomStyles.chatSurface,
      ),
    ],
    [child],
  )
}

export const workroomScrollableRoute = <Message>(
  children: ReadonlyArray<Html | string>,
): Html => {
  const h = html<Message>()

  return h.div([h.Class('py-8')], children)
}

export const workroomContent = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [...attrs, h.Class('flex h-full min-h-0 min-w-0 flex-1 flex-col')],
    children,
  )
}

export const workroomSplit = <Message>(main: Html, side: Html): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'grid h-full min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_420px] grid-rows-[minmax(0,1fr)] overflow-hidden max-[1200px]:grid-cols-[minmax(0,1fr)_380px] max-[1100px]:grid-cols-[minmax(0,1fr)]',
      ),
    ],
    [main, side],
  )
}

export const workroomTopBar = <Message>(input: {
  eyebrow: string
  title: string
  leading?: Html | string
  status?: string
  actions?: ReadonlyArray<Html | string>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const heading = `${input.eyebrow} / ${input.title}`

  return h.header(
    [
      ...(input.attrs ?? []),
      h.Class(
        clsx(
          motionPaneOpenClass,
          'flex h-12 flex-none items-center justify-between gap-3 border-b border-[#222] bg-[#010102] px-4 max-[760px]:px-3',
        ),
      ),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-center gap-3')],
        [
          input.leading ?? null,
          h.h3(
            [
              h.Class(
                'm-0 min-w-0 truncate text-sm font-medium leading-none tracking-normal text-[#f1efe8]',
              ),
            ],
            [heading],
          ),
        ],
      ),
      h.div(
        [h.Class('flex min-w-0 shrink-0 items-center gap-2')],
        [
          input.status === undefined
            ? null
            : h.div(
                [
                  h.Class(
                    'border border-[#333] px-2 py-[5px] text-[0.75rem] text-white/60',
                  ),
                ],
                [
                  h.span(
                    [h.Class(motionStatusMorphClass), h.Key(input.status)],
                    [input.status],
                  ),
                ],
              ),
          ...(input.actions ?? []),
        ],
      ),
    ],
  )
}

export const workroomPanel = <Message>(
  children: ReadonlyArray<Html | string>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [...attrs, h.Class(clsx(surfaceActiveClass, motionPaneOpenClass))],
    children,
  )
}

export const panelHeader = <Message>(input: {
  title: string
  detail?: string
  tone?: Tone
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...(input.attrs ?? []),
      h.Class('flex items-center gap-2 border-b border-[#222] px-3 py-2.5'),
    ],
    [
      h.div([h.Class(statusDotClass(input.tone ?? 'neutral'))], []),
      h.div(
        [h.Class('min-w-0 flex-1')],
        [
          h.div(
            [
              h.Class(clsx(titleClass, motionTextRevealAnimationClass)),
              h.Key(input.title),
            ],
            [input.title],
          ),
          input.detail === undefined
            ? null
            : h.div([h.Class(metaClass)], [input.detail]),
        ],
      ),
    ],
  )
}

const workroomToolTone = (
  status: Extract<WorkroomTimelinePart, { kind: 'tool' }>['status'],
): Tone => {
  if (status === 'running') {
    return 'accent'
  }

  if (status === 'completed') {
    return 'positive'
  }

  if (status === 'failed') {
    return 'negative'
  }

  return 'info'
}

const workroomChecklistTone = (state: WorkroomChecklistItem['state']): Tone => {
  if (state === 'done') {
    return 'positive'
  }

  if (state === 'active') {
    return 'accent'
  }

  return 'info'
}

export const workroomTimelinePart = <Message>(
  part: WorkroomTimelinePart,
): Html => {
  const h = html<Message>()
  const stableToolKey = (
    tool: Extract<WorkroomTimelinePart, { kind: 'tool' }>,
  ): string =>
    tool.title === 'Computer workroom'
      ? 'tool:computer-workroom'
      : `${tool.title}:${tool.subtitle}:${tool.actionHref ?? ''}`

  if (part.kind === 'text') {
    return h.div(
      [
        ...stylexAttrs<Message>(workroomStyles.textPart),
        h.DataAttribute('component', 'text-part'),
        h.DataAttribute('timeline-part-id', 'autopilot-text'),
      ],
      [
        h.div(
          [
            h.DataAttribute('slot', 'text-part-body'),
            ...stylexAttrsWithClass<Message>(
              clsx(
                'whitespace-pre-wrap break-words',
                motionTextRevealAnimationClass,
                {
                  'text-text-strong': part.tone !== 'muted',
                  'text-text-weak': part.tone === 'muted',
                },
              ),
              workroomStyles.text14Regular,
              workroomStyles.textPartBody,
            ),
            h.Key(part.body.join('\n')),
          ],
          part.body.map(line => h.p([h.Class('m-0')], [line])),
        ),
      ],
    )
  }

  if (part.kind === 'tool') {
    const isShell = part.subtitle.toLowerCase().includes('shell')
    const details =
      part.detail.length === 0
        ? []
        : [
            isShell
              ? h.div(
                  [
                    ...stylexAttrs<Message>(workroomStyles.bashOutput),
                    h.DataAttribute('component', 'bash-output'),
                  ],
                  [
                    h.div(
                      [
                        h.DataAttribute('slot', 'bash-scroll'),
                        h.DataAttribute('scrollable', ''),
                        ...stylexAttrsWithClass<Message>(
                          'border border-[#333] bg-[#050505]',
                          workroomStyles.bashScroll,
                        ),
                      ],
                      [
                        h.pre(
                          [
                            ...stylexAttrs<Message>(workroomStyles.bashPre),
                            h.DataAttribute('slot', 'bash-pre'),
                          ],
                          [
                            h.code(
                              stylexAttrs<Message>(workroomStyles.bashCode),
                              [part.detail.join('\n')],
                            ),
                          ],
                        ),
                      ],
                    ),
                  ],
                )
              : h.div(
                  [
                    h.DataAttribute('component', 'tool-output'),
                    h.DataAttribute('scrollable', ''),
                    ...stylexAttrsWithClass<Message>(
                      'border border-[#333] bg-[#050505] p-2.5',
                      workroomStyles.toolOutput,
                    ),
                  ],
                  [
                    h.pre(
                      stylexAttrs<Message>(workroomStyles.toolOutputPre),
                      part.detail.map(line =>
                        h.code(
                          [
                            h.Key(line),
                            ...stylexAttrsWithClass<Message>(
                              'block oa-text-reveal motion-reduce:animate-none',
                              workroomStyles.toolOutputCode,
                            ),
                          ],
                          [line],
                        ),
                      ),
                    ),
                  ],
                ),
          ]

    const wrapperAttrs = [
      h.DataAttribute('component', 'tool-part-wrapper'),
      h.DataAttribute('timeline-part-id', `autopilot-tool-${part.title}`),
      ...stylexAttrsWithClass<Message>(
        motionPaneOpenClass,
        workroomStyles.toolPartWrapper,
      ),
      h.Key(stableToolKey(part)),
    ]
    const action =
      part.actionHref === undefined
        ? []
        : [
            h.div(
              [
                h.DataAttribute('slot', 'tool-action'),
                h.Class('mt-3 flex justify-start'),
              ],
              [
                linkButton<Message>({
                  href: part.actionHref,
                  label: part.actionLabel ?? 'Open',
                  size: 'sm',
                  variant: 'secondary',
                }),
              ],
            ),
          ]
    const content = [
      h.div(
        [
          ...stylexAttrs<Message>(
            workroomStyles.collapsible,
            workroomStyles.toolCollapsible,
          ),
          h.DataAttribute('component', 'collapsible'),
          h.DataAttribute('variant', 'normal'),
        ],
        [
          h.div(
            [
              ...stylexAttrs<Message>(workroomStyles.collapsibleTrigger),
              h.DataAttribute('slot', 'collapsible-trigger'),
              h.AriaExpanded(details.length > 0),
            ],
            [
              h.div(
                [
                  ...stylexAttrs<Message>(workroomStyles.toolTrigger),
                  h.DataAttribute('component', 'tool-trigger'),
                ],
                [
                  h.div(
                    [
                      ...stylexAttrs<Message>(
                        workroomStyles.toolTriggerContent,
                      ),
                      h.DataAttribute(
                        'slot',
                        'basic-tool-tool-trigger-content',
                      ),
                    ],
                    [
                      h.div(
                        [
                          ...stylexAttrs<Message>(
                            workroomStyles.toolIndicator,
                          ),
                          h.DataAttribute('slot', 'basic-tool-tool-indicator'),
                        ],
                        [
                          h.span(
                            [
                              h.Class(
                                statusDotClass(workroomToolTone(part.status)),
                              ),
                            ],
                            [],
                          ),
                        ],
                      ),
                      h.div(
                        [
                          ...stylexAttrs<Message>(workroomStyles.toolInfo),
                          h.DataAttribute('slot', 'basic-tool-tool-info'),
                        ],
                        [
                          h.div(
                            [
                              ...stylexAttrs<Message>(
                                workroomStyles.toolInfoStructured,
                              ),
                              h.DataAttribute(
                                'slot',
                                'basic-tool-tool-info-structured',
                              ),
                            ],
                            [
                              h.div(
                                [
                                  ...stylexAttrs<Message>(
                                    workroomStyles.toolInfoMain,
                                  ),
                                  h.DataAttribute(
                                    'slot',
                                    'basic-tool-tool-info-main',
                                  ),
                                ],
                                [
                                  h.span(
                                    [
                                      ...stylexAttrs<Message>(
                                        workroomStyles.toolTitle,
                                      ),
                                      h.DataAttribute(
                                        'slot',
                                        'basic-tool-tool-title',
                                      ),
                                    ],
                                    [part.title],
                                  ),
                                  h.span(
                                    [
                                      ...stylexAttrs<Message>(
                                        workroomStyles.toolSubtitle,
                                      ),
                                      h.DataAttribute(
                                        'slot',
                                        'basic-tool-tool-subtitle',
                                      ),
                                    ],
                                    [part.subtitle],
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ],
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
          ...details.map(detail =>
            h.div(
              [
                ...stylexAttrs<Message>(workroomStyles.collapsibleContent),
                h.DataAttribute('slot', 'collapsible-content'),
              ],
              [detail],
            ),
          ),
          ...action,
        ],
      ),
    ]

    return part.href === undefined
      ? h.div(wrapperAttrs, content)
      : h.a(
          [
            ...wrapperAttrs,
            h.Href(part.href),
            h.AriaLabel(`${part.title}: open full thread`),
          ],
          content,
        )
  }

  if (part.kind === 'diff') {
    return h.div(
      [
        h.DataAttribute('slot', 'session-turn-diffs'),
        h.DataAttribute('component', 'session-turn-diffs-group'),
        ...stylexAttrsWithClass<Message>(
          motionPaneOpenClass,
          workroomStyles.diffs,
        ),
      ],
      [
        h.div(
          [
            ...stylexAttrs<Message>(workroomStyles.diffsHeader),
            h.DataAttribute('slot', 'session-turn-diffs-header'),
          ],
          [
            h.span(
              [
                ...stylexAttrs<Message>(workroomStyles.diffsLabel),
                h.DataAttribute('slot', 'session-turn-diffs-label'),
              ],
              [`${part.files.length} changed file`],
            ),
          ],
        ),
        h.div(
          [
            ...stylexAttrs<Message>(workroomStyles.diffsContent),
            h.DataAttribute('component', 'session-turn-diffs-content'),
          ],
          part.files.map(file =>
            h.div(
              [
                ...stylexAttrs<Message>(workroomStyles.diffTrigger),
                h.DataAttribute('slot', 'session-turn-diff-trigger'),
              ],
              [
                h.span(
                  [
                    ...stylexAttrs<Message>(workroomStyles.diffPath),
                    h.DataAttribute('slot', 'session-turn-diff-path'),
                  ],
                  [
                    h.span(
                      [
                        ...stylexAttrs<Message>(workroomStyles.diffFilename),
                        h.DataAttribute(
                          'slot',
                          'session-turn-diff-filename',
                        ),
                      ],
                      [file.path],
                    ),
                  ],
                ),
                h.div(
                  [
                    ...stylexAttrs<Message>(workroomStyles.diffMeta),
                    h.DataAttribute('slot', 'session-turn-diff-meta'),
                  ],
                  [
                    h.span(
                      [
                        ...stylexAttrsWithClass<Message>(
                          clsx({
                            'text-text-strong': file.status === 'added',
                            'text-text-weak': file.status !== 'added',
                          }),
                          workroomStyles.text12Regular,
                        ),
                      ],
                      [
                        `+${file.added}`,
                        file.removed > 0 ? ` -${file.removed}` : '',
                      ],
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ],
    )
  }

  return h.div(
    [h.DataAttribute('component', 'write-tool'), h.Class(motionPaneOpenClass)],
    [
      h.div(
        [
          ...stylexAttrs<Message>(workroomStyles.writeTrigger),
          h.DataAttribute('component', 'write-trigger'),
        ],
        [
          h.div(
            [
              ...stylexAttrs<Message>(workroomStyles.messageTitleArea),
              h.DataAttribute('slot', 'message-part-title-area'),
            ],
            [
              h.div(
                [
                  ...stylexAttrs<Message>(workroomStyles.messageTitle),
                  h.DataAttribute('slot', 'message-part-title'),
                ],
                [
                  h.span(
                    [
                      ...stylexAttrs<Message>(workroomStyles.messageTitleText),
                      h.DataAttribute('slot', 'message-part-title-text'),
                    ],
                    ['File'],
                  ),
                  h.span(
                    [
                      ...stylexAttrs<Message>(
                        workroomStyles.messageTitleFilename,
                      ),
                      h.DataAttribute('slot', 'message-part-title-filename'),
                    ],
                    [part.path],
                  ),
                ],
              ),
            ],
          ),
          h.div(
            [
              ...stylexAttrs<Message>(workroomStyles.messageActions),
              h.DataAttribute('slot', 'message-part-actions'),
            ],
            [
              h.span(
                [
                  ...stylexAttrsWithClass<Message>(
                    'text-text-weak',
                    workroomStyles.text12Regular,
                  ),
                ],
                [part.language],
              ),
            ],
          ),
        ],
      ),
      h.div(
        [
          ...stylexAttrs<Message>(workroomStyles.writeContent),
          h.DataAttribute('component', 'write-content'),
        ],
        [
          h.div(
            [
              ...stylexAttrs<Message>(workroomStyles.toolOutput),
              h.DataAttribute('component', 'tool-output'),
              h.DataAttribute('scrollable', ''),
            ],
            [
              h.pre(
                stylexAttrs<Message>(workroomStyles.toolOutputPre),
                part.excerpt.map(line =>
                  h.code(
                    stylexAttrs<Message>(workroomStyles.toolOutputCode),
                    [line, '\n'],
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

export const workroomTimelineMessage = <Message>(
  message: WorkroomTimelineMessage,
): Html => {
  const h = html<Message>()
  const isUser = message.author === 'user'
  const isSystem = message.author === 'system'
  const userTextParts = message.parts.filter(part => part.kind === 'text')
  const userNonTextParts = message.parts.filter(part => part.kind !== 'text')
  const userTextBody = userTextParts.flatMap(part => part.body).join('\n')
  const messageContent = isUser
    ? h.div(
        [
          h.DataAttribute('slot', 'session-turn-user-content'),
          h.Class('grid w-full justify-items-end gap-3'),
        ],
        [
          userTextParts.length === 0
            ? null
            : h.div(
                [
                  h.DataAttribute('component', 'user-message-with-author'),
                  h.Class(
                    'w-full max-w-[min(100%,54rem)] justify-self-end md:max-w-[min(88%,58rem)] 2xl:max-w-[min(78%,68rem)]',
                  ),
                ],
                [
                  v4ChatMessage<Message>({
                    author: message.label,
                    ...(message.avatarUrl === undefined ||
                    message.avatarUrl === ''
                      ? {}
                      : { avatarUrl: message.avatarUrl }),
                    body: userTextBody,
                    attrs: [
                      h.DataAttribute('component', 'user-message'),
                      h.DataAttribute('slot', 'user-message-body'),
                    ],
                  }),
                ],
              ),
          ...userNonTextParts.map(part => workroomTimelinePart<Message>(part)),
        ],
      )
    : h.div(
        [
          ...stylexAttrs<Message>(workroomStyles.assistantContent),
          h.DataAttribute('slot', 'session-turn-assistant-content'),
        ],
        [
          ...message.parts.map(part => workroomTimelinePart<Message>(part)),
          ...(message.status === 'streaming'
            ? [
                h.span(
                  [
                    h.Class(
                      clsx(
                        motionStatusMorphAnimationClass,
                        'ml-1 inline-block h-[15px] w-[7px] translate-y-0.5 animate-pulse bg-[#ffb400] motion-reduce:animate-none',
                      ),
                    ),
                  ],
                  [],
                ),
              ]
            : []),
        ],
      )

  return h.article(
    [
      h.Id(`message-${message.id}`),
      h.DataAttribute('author', message.author),
      h.DataAttribute(
        'timeline-row',
        isUser ? 'UserMessage' : isSystem ? 'TurnDivider' : 'AssistantPart',
      ),
      h.Class(
        clsx(motionPaneOpenClass, 'min-w-0 w-full max-w-full', {
          'md:max-w-200 2xl:max-w-[1000px] md:mx-auto': !isUser,
        }),
      ),
    ],
    [
      h.div(
        [
          h.DataAttribute('component', 'session-turn'),
          ...stylexAttrsWithClass<Message>(
            'min-w-0 w-full relative',
            workroomStyles.sessionTurn,
          ),
        ],
        [
          h.div(
            [
              h.DataAttribute('slot', 'session-turn-message-container'),
              ...stylexAttrsWithClass<Message>(
                'w-full px-4 md:px-5',
                workroomStyles.sessionMessageContainer,
              ),
            ],
            [
              h.div(
                [
                  ...stylexAttrs<Message>(
                    workroomStyles.sessionMessageContent,
                  ),
                  h.DataAttribute('slot', 'session-turn-message-content'),
                  h.AriaLive('off'),
                ],
                [messageContent],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

export const workroomTimeline = <Message>(input: {
  messages: ReadonlyArray<WorkroomTimelineMessage>
  endMarker?: Html
}): Html => {
  const h = html<Message>()
  const isActive = input.messages.some(
    message => message.status === 'streaming',
  )

  return h.section(
    [
      kitFamily<Message>('lists/feeds'),
      h.AriaLabel('Autopilot conversation'),
      h.Class(
        'relative h-full min-h-0 min-w-0 flex-1 overflow-hidden bg-[#000]',
      ),
    ],
    [
      h.div(
        [
          h.Class(
            clsx(
              'absolute inset-x-0 top-0 z-[2] h-px overflow-hidden',
              'oa-progress-strip',
              { 'is-active': isActive },
            ),
          ),
        ],
        [h.span([], [])],
      ),
      h.div(
        [
          h.Class(
            'absolute inset-0 flex min-h-0 flex-col gap-[26px] overflow-auto overscroll-contain px-[clamp(16px,4vw,56px)] pb-10 pt-7 max-[760px]:px-3 max-[760px]:pb-7 max-[760px]:pt-[18px]',
          ),
        ],
        [
          ...input.messages.map(message =>
            workroomTimelineMessage<Message>(message),
          ),
          input.endMarker ?? null,
        ],
      ),
    ],
  )
}

export const workroomTimelineEndMarker = <Message>(
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div([...attrs, h.Class('h-0')], [])
}

export const workroomChecklist = <Message>(input: {
  title: string
  items: ReadonlyArray<WorkroomChecklistItem>
  meta?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return workroomPanel<Message>(
    [
      h.div(
        [
          h.Class(
            'flex items-center justify-between gap-3 border-b border-[#222] px-2.5 py-2 text-[0.75rem] text-white/60',
          ),
        ],
        [
          h.span([], [input.title]),
          input.meta === undefined ? null : h.span([], [input.meta]),
        ],
      ),
      h.ul(
        [
          kitFamily<Message>('lists/stacked-lists'),
          h.Role('list'),
          h.Class('m-0 grid list-none gap-1.5 px-2.5 pb-2.5 pt-2'),
        ],
        input.items.map(item =>
          h.li(
            [h.Class(clsx(rowClass, 'text-[0.75rem] text-white/60'))],
            [
              h.span(
                [h.Class(statusDotClass(workroomChecklistTone(item.state)))],
                [],
              ),
              h.span([], [item.label]),
            ],
          ),
        ),
      ),
    ],
    input.attrs ?? [],
  )
}

export const workroomActionDock = <Message>(input: {
  title: string
  rows: ReadonlyArray<Readonly<{ label: string; action: Html }>>
  meta?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return workroomPanel<Message>(
    [
      h.div(
        [
          h.Class(
            'flex items-center justify-between gap-3 border-b border-[#222] px-2.5 py-2 text-[0.75rem] text-white/60',
          ),
        ],
        [
          h.span([], [input.title]),
          input.meta === undefined ? null : h.span([], [input.meta]),
        ],
      ),
      h.div(
        [h.Class('grid gap-1.5 px-2.5 pb-2.5 pt-2')],
        input.rows.map(row =>
          h.div(
            [
              h.Class(
                clsx(
                  rowClass,
                  'justify-between text-[0.8125rem] text-[#f1efe8] [&>span]:min-w-0 [&>span]:overflow-hidden [&>span]:text-ellipsis [&>span]:whitespace-nowrap',
                ),
              ),
            ],
            [h.span([], [row.label]), row.action],
          ),
        ),
      ),
    ],
    input.attrs ?? [],
  )
}

export const workroomPermissionDock = <Message>(input: {
  title: string
  body: string
  code?: string
  actions: ReadonlyArray<Html>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [...(input.attrs ?? []), h.Class('grid')],
    [
      workroomPanel<Message>([
        panelHeader<Message>({
          title: input.title,
          tone: 'negative',
        }),
        h.div(
          [
            h.Class(
              'grid gap-2 px-3 py-2.5 text-[0.8125rem] text-white/60 [&_code]:break-words [&_code]:text-[#ffb400] [&_p]:m-0',
            ),
          ],
          [
            h.p([], [input.body]),
            input.code === undefined ? null : h.code([], [input.code]),
          ],
        ),
      ]),
      h.div(
        [
          h.Class(
            'flex items-center justify-between gap-3 border border-t-0 border-[#222] bg-[#080808] px-3 py-2.5 max-[760px]:w-full max-[760px]:flex-wrap',
          ),
        ],
        [
          h.div([], []),
          h.div([h.Class('flex min-w-0 items-center gap-2')], input.actions),
        ],
      ),
    ],
  )
}

export const workroomComposer = <Message>(input: {
  textareaId?: string
  value: string
  isStreaming: boolean
  canSubmit: boolean
  onSubmit: Attribute<Message>
  onInput: Attribute<Message>
  onKeyDown?: Attribute<Message>
  actions?: ReadonlyArray<Html>
  followups?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.form(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('forms/textareas'),
      h.Class(
        clsx(
          motionPaneOpenClass,
          'relative z-10 grid flex-none gap-2 bg-transparent px-3 pb-2.5 pt-0',
        ),
      ),
      input.onSubmit,
    ],
    [
      input.followups ?? null,
      h.div(
        [h.Class('mx-auto flex w-full max-w-[768px] flex-col justify-end')],
        [
          h.div(
            [h.Class('flex w-full flex-col')],
            [
              h.div(
                [
                  h.Class(
                    'grid border border-[#222] bg-transparent shadow-[0_0_0_1px_rgba(255,255,255,0.02)]',
                  ),
                ],
                [
                  h.div(
                    [h.Class('flex min-h-[43px] flex-row')],
                    [
                      h.div(
                        [h.Class('flex flex-1 items-center justify-center')],
                        [
                          h.div(
                            [h.Class('w-full min-w-0 px-3 py-2')],
                            [
                              h.textarea(
                                [
                                  ...(input.textareaId === undefined
                                    ? []
                                    : [h.Id(input.textareaId)]),
                                  h.Name('chat-prompt'),
                                  h.AriaLabel('Message Autopilot'),
                                  h.Placeholder(
                                    input.isStreaming
                                      ? 'Autopilot run is active...'
                                      : 'Type your message...',
                                  ),
                                  h.Value(input.value),
                                  h.Rows(1),
                                  h.Class(
                                    'min-h-[27px] w-full resize-none border-0 bg-transparent p-0 font-[inherit] text-[0.8125rem] leading-5 text-[#f1efe8] outline-none placeholder:text-white/35',
                                  ),
                                  input.onInput,
                                  ...(input.onKeyDown === undefined
                                    ? []
                                    : [input.onKeyDown]),
                                ],
                                [],
                              ),
                            ],
                          ),
                        ],
                      ),
                      h.div([h.Class('m-2 flex self-end gap-0.5')], []),
                    ],
                  ),
                  h.div(
                    [
                      h.Class(
                        'flex flex-row justify-between gap-3 border-t border-[#222] p-2 max-[760px]:w-full max-[760px]:flex-wrap',
                      ),
                    ],
                    [
                      (input.actions ?? []).length === 0
                        ? h.div([], [])
                        : h.div(
                            [
                              h.Class(
                                'flex min-w-0 flex-wrap items-center gap-x-1 gap-y-2',
                              ),
                            ],
                            input.actions ?? [],
                          ),
                      h.button(
                        [
                          h.Type('submit'),
                          h.Disabled(!input.canSubmit),
                          h.Class(
                            clsx(
                              motionRowHoverClass,
                              'inline-flex h-[26px] shrink-0 cursor-pointer items-center border border-[#333] bg-[#080808] px-2 font-[inherit] text-[0.75rem] leading-none text-[#f1efe8] hover:border-[#ffb400] hover:bg-[#141414] hover:text-[#f1efe8]',
                              {
                                'cursor-not-allowed border-[#222] bg-[#080808] text-white/35 hover:bg-[#080808]':
                                  !input.canSubmit,
                              },
                            ),
                          ),
                        ],
                        [input.isStreaming ? 'Running' : 'Send'],
                      ),
                    ],
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

export const workroomComposerModeSelect = <Message>(): Html => {
  const h = html<Message>()

  return h.select(
    [
      h.AriaLabel('Composer mode'),
      h.Name('composer-mode'),
      h.Class(
        'mr-1 h-[26px] rounded-[4px] border border-[#333] bg-[#080808] px-2 font-[inherit] text-[0.75rem] leading-none text-white/60 outline-none hover:bg-[#141414] hover:text-[#f1efe8] focus:border-[#ffb400] focus:ring-1 focus:ring-[#ffb400]',
      ),
    ],
    [
      h.option([h.Value('chat'), h.Selected(true)], ['Chat']),
      h.option([h.Value('codex')], ['Codex']),
      h.option([h.Value('opencode')], ['OpenCode']),
    ],
  )
}

export const workroomFilePanel = <Message>(input: {
  tabs: ReadonlyArray<WorkroomTab>
  rows: ReadonlyArray<KeyValueItem>
  files: ReadonlyArray<WorkroomFileItem>
  docks?: ReadonlyArray<Html>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.aside(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('page-examples/detail-screens'),
      h.AriaLabel('Files and review'),
      h.Class(
        clsx(
          motionPaneOpenClass,
          'flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l border-[#222] bg-[#020202] max-[1100px]:hidden',
        ),
      ),
    ],
    [
      input.tabs.length === 0 ? null : tabBar<Message>(input.tabs),
      keyValueRows<Message>(input.rows),
      h.div(
        [
          kitFamily<Message>('lists/tables'),
          h.Class(
            'grid min-h-0 flex-1 content-start gap-0.5 overflow-auto overscroll-contain p-2.5',
          ),
        ],
        input.files.map(file =>
          h.div(
            [
              h.Class(
                clsx(
                  rowClass,
                  'justify-between border p-2 [&>span:first-child]:text-[0.8125rem] [&>span:first-child]:text-[#f1efe8] [&>span:last-child]:overflow-hidden [&>span:last-child]:text-ellipsis [&>span:last-child]:whitespace-nowrap [&>span:last-child]:text-[0.75rem] [&>span:last-child]:text-white/35',
                  motionRowHoverClass,
                  {
                    'border-[#333] bg-[#080808]': file.active === true,
                    'border-transparent': file.active !== true,
                    'pl-6': file.depth === 1,
                  },
                ),
              ),
            ],
            [h.span([], [file.label]), h.span([], [file.meta])],
          ),
        ),
      ),
      h.div(
        [h.Class('grid flex-none gap-2.5 border-t border-[#222] p-2.5')],
        input.docks ?? [],
      ),
    ],
  )
}

export const workroomPanelActionRow = <Message>(input: {
  label: string
  action: Html
}): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class('flex items-center justify-between gap-2')],
    [
      h.span([h.Class('text-[0.75rem] text-white/45')], [input.label]),
      input.action,
    ],
  )
}

export const workroomTerminalPanel = <Message>(input: {
  tabs: ReadonlyArray<string>
  lines: ReadonlyArray<string>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('lists/feeds'),
      h.Class(
        'flex-none border-t border-[#222] bg-[#030303] max-[760px]:hidden',
      ),
    ],
    [
      h.div(
        [
          h.Class(
            'flex min-h-9 items-center justify-start gap-3 border-b border-[#222] px-2',
          ),
        ],
        input.tabs.map((tab, index) =>
          h.div(
            [
              h.Class(
                clsx(rowClass, 'h-[26px] border px-2 text-[0.75rem]', {
                  'border-[#333] bg-[#080808] text-[#f1efe8]': index === 0,
                  'border-transparent text-white/60': index !== 0,
                }),
              ),
            ],
            [tab],
          ),
        ),
      ),
      codeBlock<Message>({
        maxHeightClass: 'max-h-28',
        lines: input.lines,
      }),
    ],
  )
}

export const workroomMetadataDialog = <Message>(input: {
  ariaLabel: string
  eyebrow: string
  title: string
  body: string
  actions: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('overlays/modal-dialogs'),
      h.Role('dialog'),
      h.AriaModal(true),
      h.AriaLabel(input.ariaLabel),
      h.Class(
        clsx(
          motionPaneOpenClass,
          'fixed inset-0 z-50 grid place-items-center bg-black/75 p-4 backdrop-blur-sm',
        ),
      ),
    ],
    [
      h.div(
        [
          h.Class(
            clsx(
              motionPaneOpenClass,
              'grid max-h-[min(760px,92vh)] w-[min(920px,94vw)] grid-rows-[auto_minmax(0,1fr)] border border-[#333] bg-[#010102]',
            ),
          ),
        ],
        [
          h.div(
            [
              h.Class(
                'flex items-center justify-between gap-3 border-b border-[#222] px-4 py-3',
              ),
            ],
            [
              h.div(
                [h.Class('min-w-0')],
                [
                  h.div(
                    [h.Class('text-[0.6875rem] uppercase text-white/35')],
                    [input.eyebrow],
                  ),
                  h.h2([h.Class('m-0 text-sm text-[#f1efe8]')], [input.title]),
                ],
              ),
              buttonGroup<Message>(input.actions),
            ],
          ),
          h.pre(
            [
              h.Class(
                'm-0 min-h-0 overflow-auto whitespace-pre-wrap break-words p-4 text-[0.75rem] leading-5 text-white/65',
              ),
            ],
            [input.body],
          ),
        ],
      ),
    ],
  )
}
