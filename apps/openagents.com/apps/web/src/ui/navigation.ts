import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { badge } from './data-display'
import { inputClass } from './forms'
import { container } from './layout'
import { headingBlock } from './shared'
import {
  eyebrowClass,
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceActiveClass,
  surfaceClass,
  titleClass,
} from './primitives'
import type { NavItem, ProgressStep, WorkroomTab } from './primitives'

export const tabBar = <Message>(tabs: ReadonlyArray<WorkroomTab>): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'flex items-center justify-start gap-3 border-b border-[#222] p-2',
      ),
    ],
    tabs.map(tab =>
      h.button(
        [
          h.Type('button'),
          h.Class(
            clsx(
              'min-h-[30px] cursor-pointer border border-[#333] px-2.5 font-[inherit] text-[0.75rem] hover:bg-[#080808] hover:text-[#f1efe8]',
              {
                'bg-[#080808] text-[#f1efe8]': tab.active === true,
                'bg-transparent text-white/60': tab.active !== true,
              },
            ),
          ),
        ],
        [tab.label],
      ),
    ),
  )
}

export const navBar = <Message>(
  items: ReadonlyArray<NavItem>,
  trailing?: Html | string,
): Html => {
  const h = html<Message>()

  return h.nav(
    [h.Class('border-b border-[#222] bg-[#010102] text-[#f1efe8]')],
    [
      container<Message>(
        [
          h.ul(
            [h.Role('list'), h.Class('m-0 flex list-none gap-2 p-0')],
            items.map(item =>
              h.li(
                [],
                [
                  h.a(
                    [
                      h.Href(item.href),
                      h.Class(
                        clsx(
                          'inline-flex min-h-8 items-center gap-2 border border-transparent px-2.5 text-sm text-white/60 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
                          {
                            'border-[#333] bg-[#080808] text-[#f1efe8]':
                              item.active === true,
                          },
                        ),
                      ),
                    ],
                    [
                      item.label,
                      item.meta === undefined
                        ? null
                        : h.em(
                            [h.Class('font-normal not-italic text-white/35')],
                            [item.meta],
                          ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          trailing ?? null,
        ],
        [
          h.Class(
            'mx-auto flex w-[min(100%,1120px)] items-center justify-between gap-4 px-4 py-3',
          ),
        ],
      ),
    ],
  )
}

export const navSessionMeta = <Message>(sessionLabel: string): Html => {
  const h = html<Message>()

  return h.div(
    [kitFamily<Message>('navigation/navbars'), h.Class(metaClass)],
    [sessionLabel],
  )
}

export const breadcrumbBar = <Message>(items: ReadonlyArray<NavItem>): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('navigation/breadcrumbs'),
      h.AriaLabel('Breadcrumb'),
      h.Class('overflow-x-auto whitespace-nowrap'),
    ],
    [
      h.ol(
        [h.Role('list'), h.Class('m-0 flex list-none items-center gap-2 p-0')],
        items.map((item, index) =>
          h.li(
            [h.Class('flex items-center gap-2 text-sm text-white/45')],
            [
              index === 0 ? null : h.span([h.AriaHidden(true)], ['/']),
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    clsx('no-underline hover:text-[#f1efe8]', {
                      'text-[#f1efe8]': item.active === true,
                      'text-white/45': item.active !== true,
                    }),
                  ),
                ],
                [item.label],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

export const pagination = <Message>(input: {
  previousHref?: string
  nextHref?: string
  pages: ReadonlyArray<NavItem>
}): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('navigation/pagination'),
      h.AriaLabel('Pagination'),
      h.Class(
        'flex min-w-0 flex-wrap items-center justify-between gap-3 border-t border-[#222] py-3',
      ),
    ],
    [
      input.previousHref === undefined
        ? h.span([h.Class('text-sm text-white/25')], ['Previous'])
        : h.a(
            [
              h.Href(input.previousHref),
              h.Class(
                'text-sm text-white/60 no-underline hover:text-[#f1efe8]',
              ),
            ],
            ['Previous'],
          ),
      h.ol(
        [h.Role('list'), h.Class('m-0 flex list-none gap-1 p-0')],
        input.pages.map(page =>
          h.li(
            [],
            [
              h.a(
                [
                  h.Href(page.href),
                  h.Class(
                    clsx(
                      'grid h-8 min-w-8 place-items-center border px-2 text-sm no-underline',
                      {
                        'border-[#333] bg-[#141414] text-[#f1efe8]':
                          page.active === true,
                        'border-transparent text-white/45 hover:border-[#333] hover:text-[#f1efe8]':
                          page.active !== true,
                      },
                    ),
                  ),
                ],
                [page.label],
              ),
            ],
          ),
        ),
      ),
      input.nextHref === undefined
        ? h.span([h.Class('text-sm text-white/25')], ['Next'])
        : h.a(
            [
              h.Href(input.nextHref),
              h.Class(
                'text-sm text-white/60 no-underline hover:text-[#f1efe8]',
              ),
            ],
            ['Next'],
          ),
    ],
  )
}

export const verticalNavigation = <Message>(
  items: ReadonlyArray<NavItem>,
): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('navigation/vertical-navigation'),
      h.Class('grid gap-1'),
    ],
    items.map(item =>
      h.a(
        [
          h.Href(item.href),
          h.Class(
            clsx(
              'grid min-h-9 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent px-2.5 text-sm text-white/60 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
              {
                'border-[#333] bg-[#141414] text-[#f1efe8]':
                  item.active === true,
              },
            ),
          ),
        ],
        [
          h.span([h.Class('truncate')], [item.label]),
          item.meta === undefined ? null : badge<Message>({ label: item.meta }),
        ],
      ),
    ),
  )
}

export const sidebarNavigation = <Message>(input: {
  brand: string
  items: ReadonlyArray<NavItem>
  footer?: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.aside(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('navigation/sidebar-navigation'),
      h.Class(
        'hidden min-h-0 w-64 flex-col border-r border-[#222] bg-[#010102] lg:flex',
      ),
    ],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-4')],
        [
          h.p([h.Class(clsx(eyebrowClass, 'mb-1'))], ['OpenAgents']),
          h.h2(
            [h.Class('m-0 text-lg font-medium text-[#f1efe8]')],
            [input.brand],
          ),
        ],
      ),
      h.div(
        [h.Class('min-h-0 flex-1 overflow-auto overscroll-contain p-3')],
        [verticalNavigation<Message>(input.items)],
      ),
      input.footer === undefined
        ? null
        : h.div([h.Class('border-t border-[#222] p-3')], [input.footer]),
    ],
  )
}

export const commandPalette = <Message>(input: {
  title: string
  placeholder: string
  items: ReadonlyArray<NavItem>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('navigation/command-palettes'),
      h.Class(clsx(surfaceActiveClass, 'grid gap-0')),
    ],
    [
      h.div(
        [h.Class('border-b border-[#222] px-3 py-2.5')],
        [
          headingBlock<Message>({
            eyebrow: 'Command',
            title: input.title,
            level: 3,
          }),
        ],
      ),
      h.input([
        h.Type('search'),
        h.Name('command'),
        h.AriaLabel(input.placeholder),
        h.Placeholder(input.placeholder),
        h.Class(clsx(inputClass, 'border-0 border-b border-[#222]')),
      ]),
      h.ul(
        [h.Role('list'), h.Class('m-0 grid list-none p-0')],
        input.items.map(item =>
          h.li(
            [h.Class('border-b border-[#222] last:border-b-0')],
            [
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    'grid gap-0.5 px-3 py-2.5 text-white/70 no-underline hover:bg-[#080808] hover:text-[#f1efe8]',
                  ),
                ],
                [
                  h.span([h.Class(titleClass)], [item.label]),
                  item.meta === undefined
                    ? null
                    : h.span([h.Class(metaClass)], [item.meta]),
                ],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

export const progressList = <Message>(
  steps: ReadonlyArray<ProgressStep>,
): Html => {
  const h = html<Message>()

  return h.ol(
    [
      kitFamily<Message>('navigation/progress-bars'),
      h.Class('m-0 grid list-none border border-[#222] bg-[#010102] p-0'),
    ],
    steps.map((step, index) =>
      h.li(
        [
          h.Class(
            clsx(
              'grid min-w-0 grid-cols-[3rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-[#222] px-3 py-3 last:border-b-0',
              { 'bg-[#080808]': step.active === true },
            ),
          ),
        ],
        [
          h.span(
            [h.Class('text-xs text-white/35')],
            [String(index + 1).padStart(2, '0')],
          ),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(titleClass)], [step.label]),
              step.detail === undefined
                ? null
                : h.p([h.Class(metaClass)], [step.detail]),
            ],
          ),
          h.span([h.Class(statusDotClass(step.tone ?? 'neutral'))], []),
        ],
      ),
    ),
  )
}

export const storeNavigation = <Message>(
  items: ReadonlyArray<NavItem>,
): Html => {
  const h = html<Message>()

  return h.nav(
    [h.Class(clsx(surfaceClass, 'overflow-hidden'))],
    [
      h.ul(
        [
          h.Role('list'),
          h.Class('m-0 grid list-none p-0 sm:grid-cols-2 lg:grid-cols-4'),
        ],
        items.map(item =>
          h.li(
            [h.Class('border-b border-r border-[#222] last:border-r-0')],
            [
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    clsx(
                      'grid min-h-20 gap-1 p-4 text-white/60 no-underline hover:bg-[#080808] hover:text-[#f1efe8]',
                      { 'bg-[#141414] text-[#f1efe8]': item.active === true },
                    ),
                  ),
                ],
                [
                  h.span([h.Class(titleClass)], [item.label]),
                  item.meta === undefined
                    ? null
                    : h.em(
                        [h.Class('text-xs not-italic text-white/35')],
                        [item.meta],
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
