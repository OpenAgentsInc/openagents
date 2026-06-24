import { clsx } from 'clsx'
import type { Attribute } from 'foldkit/html'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { iconView } from './icon'
import { button } from './shared'
import { kitFamily, motionPaneOpenClass } from './primitives'

// ---------------------------------------------------------------------------
// Pro operator console primitives (issue 6179)
// ---------------------------------------------------------------------------
//
// Command surfaces — strips, registers, panes — for the /pro power-user operator
// console. Dark-only pure black, panels #010102, subtle #222 borders, warm
// off-white #f1efe8, mono-first. Semantic accents for STATE only. No cards, no
// hero, no gradients, no eyebrows (apps/openagents.com/DESIGN.md).
//
// These are the shared, class-bearing registry components so the app page stays
// composed through the Foldkit UI system rather than carrying raw Tailwind.

export type ProConsoleSection = Readonly<{
  // Section label, e.g. 'Overview'.
  label: string
  // The single live section is active; the rest are honest disabled
  // placeholders (rendered as non-interactive, never fake links).
  active?: boolean
  disabled?: boolean
  // href is only used for an active, live section.
  href?: string
}>

// Top strip: compact, mono, 1px subtle bottom border. Left = PRO mark + a thin
// breadcrumb. Right = a QUIET usage/credits indicator placeholder + account.
export const proTopStrip = <Message>(input: {
  homeHref: string
  breadcrumb: string
  // The credits/usage indicator is an honest placeholder: it never shows a real
  // balance. `creditsHint` is the title shown on hover.
  creditsLabel: string
  creditsState: string
  creditsHint: string
  accountLabel: string
}): Html => {
  const h = html<Message>()

  return h.header(
    [
      kitFamily<Message>('navigation/navbars'),
      h.DataAttribute('component', 'pro-top-strip'),
      h.Class(
        'flex min-h-11 items-center justify-between gap-4 border-b border-[#222] bg-[#010102] px-4 text-sm',
      ),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-center gap-3')],
        [
          h.a(
            [
              h.Href(input.homeHref),
              h.AriaLabel('Pro console'),
              h.Class(
                'font-semibold uppercase tracking-[0.18em] text-[#f1efe8] no-underline',
              ),
            ],
            ['PRO'],
          ),
          h.span([h.AriaHidden(true), h.Class('text-white/25')], ['/']),
          h.span([h.Class('truncate text-white/45')], [input.breadcrumb]),
        ],
      ),
      h.div(
        [h.Class('flex shrink-0 items-center gap-3')],
        [
          h.span(
            [
              h.DataAttribute('component', 'pro-credits-indicator'),
              h.Title(input.creditsHint),
              h.Class(
                'inline-flex items-center gap-1.5 border border-dashed border-white/15 px-2 py-0.5 text-[0.6875rem] uppercase tracking-[0.08em] text-white/35',
              ),
            ],
            [
              h.span([h.Class('text-white/25')], [input.creditsLabel]),
              h.span([h.Class('text-white/45')], [input.creditsState]),
            ],
          ),
          h.span(
            [h.Class('hidden truncate text-white/45 sm:inline')],
            [input.accountLabel],
          ),
        ],
      ),
    ],
  )
}

const proRegisterItem = <Message>(section: ProConsoleSection): Html => {
  const h = html<Message>()

  if (section.disabled === true || section.href === undefined) {
    return h.div(
      [
        h.AriaDisabled(true),
        h.Title(`${section.label} is coming to Pro.`),
        h.Class(
          'grid cursor-default grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent px-2.5 py-1.5 text-sm text-white/25',
        ),
      ],
      [
        h.span([h.Class('truncate')], [section.label]),
        h.span(
          [h.Class('text-[0.625rem] uppercase tracking-[0.08em] text-white/20')],
          ['soon'],
        ),
      ],
    )
  }

  const activeAttrs: ReadonlyArray<Attribute<Message>> =
    section.active === true ? [h.AriaCurrent('page')] : []

  return h.a(
    [
      h.Href(section.href),
      ...activeAttrs,
      h.Class(
        clsx(
          'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent px-2.5 py-1.5 text-sm no-underline',
          'transition-[border-color,background-color,color] duration-150 motion-reduce:transition-none',
          {
            'border-[#333] bg-[#141414] text-[#f1efe8]': section.active === true,
            'text-white/60 hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8] focus-visible:border-[#333] focus-visible:text-[#f1efe8] focus-visible:outline-none':
              section.active !== true,
          },
        ),
      ),
    ],
    [h.span([h.Class('truncate')], [section.label])],
  )
}

// Left register/nav: narrow vertical list, muted -> active. The live section is
// active; the rest are honest disabled placeholders pointing at future features.
export const proRegister = <Message>(
  sections: ReadonlyArray<ProConsoleSection>,
): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('navigation/vertical-navigation'),
      h.DataAttribute('component', 'pro-register'),
      h.AriaLabel('Pro sections'),
      h.Class(
        'grid auto-rows-min content-start gap-0.5 border-r border-[#222] bg-[#010102] p-2 sm:w-44',
      ),
    ],
    sections.map(section => proRegisterItem<Message>(section)),
  )
}

// A disabled forward affordance rendered honestly: it looks like an action but
// is non-interactive and labelled with its hint. Never a working button.
export const proComingAffordance = <Message>(input: {
  label: string
  hint: string
}): Html => {
  const h = html<Message>()

  return button<Message>({
    label: input.label,
    variant: 'secondary',
    size: 'sm',
    attrs: [
      h.Disabled(true),
      h.AriaDisabled(true),
      h.Title(input.hint),
      h.DataAttribute('state', 'coming'),
      h.Class('cursor-not-allowed opacity-50'),
    ],
  })
}

// The Overview EMPTY STATE that teaches: one line on what Pro is, plus the
// forward affordances (passed in) shown honestly. No hero, no cards.
export const proTeachingEmptyState = <Message>(input: {
  title: string
  body: string
  footnote: string
  affordances: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-empty'),
      h.Class(motionPaneOpenClass),
    ],
    [
      h.div(
        [
          h.Class(
            'mx-auto grid max-w-[64ch] gap-5 border border-[#222] bg-[#010102] p-6',
          ),
        ],
        [
          h.div(
            [h.Class('grid gap-2')],
            [
              h.h1(
                [
                  h.Class(
                    'm-0 text-base font-semibold tracking-[0.01em] text-[#f1efe8]',
                  ),
                ],
                [input.title],
              ),
              h.p(
                [h.Class('m-0 text-sm leading-[1.6] text-white/60')],
                [input.body],
              ),
            ],
          ),
          h.div(
            [
              h.Class(
                'flex flex-wrap items-center gap-2 border-t border-[#222] pt-4',
              ),
            ],
            input.affordances,
          ),
          h.p(
            [h.Class('m-0 text-xs leading-[1.55] text-white/35')],
            [input.footnote],
          ),
        ],
      ),
    ],
  )
}

// LOADING STATE: skeleton rows (not a center spinner), reduced-motion aware.
export const proSkeletonRows = <Message>(
  widthClasses: ReadonlyArray<string> = ['w-2/3', 'w-1/2', 'w-3/5', 'w-2/5'],
): Html => {
  const h = html<Message>()
  const row = (widthClass: string): Html =>
    h.div(
      [h.Class('flex items-center gap-3 border border-[#222] bg-[#010102] p-3')],
      [
        h.span(
          [
            h.Class(
              'block h-3 w-3 shrink-0 animate-pulse bg-white/15 motion-reduce:animate-none',
            ),
          ],
          [],
        ),
        h.span(
          [
            h.Class(
              clsx(
                'block h-3 animate-pulse bg-white/12 motion-reduce:animate-none',
                widthClass,
              ),
            ),
          ],
          [],
        ),
      ],
    )

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-loading'),
      h.AriaBusy(true),
      h.Class('mx-auto grid max-w-[64ch] gap-2'),
    ],
    widthClasses.map(row),
  )
}

// ERROR STATE: a compact inline error strip (not a full-page error).
export const proErrorStrip = <Message>(detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-error'),
      h.Role('alert'),
      h.Class(
        'mx-auto flex max-w-[64ch] items-center gap-3 border border-[#d32f2f]/70 bg-[#010102] px-3 py-2 text-sm text-[#f1efe8]',
      ),
    ],
    [
      iconView<Message>('Warning', 'size-4 text-[#d32f2f]'),
      h.span([h.Class('min-w-0 flex-1 truncate text-white/70')], [detail]),
    ],
  )
}

// The Pro console shell: top-strip + left-register + main pane. Owns the full
// layout independent of the workroom sidebar shell.
export const proConsoleShell = <Message>(input: {
  topStrip: Html
  register: Html
  main: Html
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('application-shells/stacked'),
      h.DataAttribute('component', 'pro-console'),
      h.Class('grid h-dvh grid-rows-[auto_minmax(0,1fr)] bg-[#000]'),
    ],
    [
      input.topStrip,
      h.div(
        [h.Class('grid min-h-0 grid-cols-[auto_minmax(0,1fr)]')],
        [input.register, input.main],
      ),
    ],
  )
}

// The scrollable main pane wrapper for the console body.
export const proMainPane = <Message>(children: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('component', 'pro-main-pane'),
      h.Class('min-w-0 overflow-y-auto p-4'),
    ],
    children,
  )
}
