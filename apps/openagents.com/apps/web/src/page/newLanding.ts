import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'

const navItems = ['Docs', 'Components', 'Blocks', 'Illustrations', 'Templates', 'Pricing']

const logoMark = <Message,>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid size-8 place-items-center rounded-[4px] border border-white/15 bg-white/[0.06] text-sm font-semibold text-white shadow-[0_0_28px_rgba(37,99,235,0.34)]',
      ),
    ],
    ['OA'],
  )
}

const iconButton = <Message,>(label: string, glyph: string): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href('#'),
      h.AriaLabel(label),
      Ui.className<Message>(
        'grid size-9 place-items-center rounded-[4px] border border-white/10 bg-white/[0.03] text-sm text-white/58 transition hover:border-white/20 hover:bg-white/[0.06] hover:text-white',
      ),
    ],
    [glyph],
  )
}

const avatar = <Message,>(label: string, color: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      h.AriaLabel(label),
      Ui.className<Message>(
        `grid size-7 place-items-center rounded-full border-2 border-[#05060a] text-[0.62rem] font-semibold text-white ${color}`,
      ),
    ],
    [label.slice(0, 1)],
  )
}

const dashboardMetric = <Message,>(label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'rounded-[4px] border border-white/8 bg-white/[0.035] p-3',
      ),
    ],
    [
      h.p([Ui.className<Message>('m-0 text-[0.62rem] text-white/38')], [label]),
      h.p([Ui.className<Message>('m-0 mt-2 text-xl font-semibold text-white')], [
        value,
      ]),
    ],
  )
}

const dashboardMockup = <Message,>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'relative mx-auto mt-10 w-full max-w-6xl px-3 pb-2 sm:mt-14',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'absolute inset-x-4 bottom-0 top-8 rounded-full bg-[#2563eb]/30 blur-3xl',
          ),
        ],
        [],
      ),
      h.div(
        [
          Ui.className<Message>(
            'relative overflow-hidden rounded-[6px] border border-white/12 bg-[#05060a] shadow-[0_36px_120px_rgba(37,99,235,0.34),0_18px_60px_rgba(0,0,0,0.75)] [transform:perspective(1100px)_rotateX(13deg)] [transform-origin:top_center]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex h-10 items-center gap-2 border-b border-white/10 bg-white/[0.035] px-4',
              ),
            ],
            [
              h.span([Ui.className<Message>('size-2 rounded-full bg-[#ff5f57]')], []),
              h.span([Ui.className<Message>('size-2 rounded-full bg-[#febc2e]')], []),
              h.span([Ui.className<Message>('size-2 rounded-full bg-[#28c840]')], []),
              h.span(
                [
                  Ui.className<Message>(
                    'ml-3 h-5 w-40 rounded-[4px] border border-white/8 bg-black/35',
                  ),
                ],
                [],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-4 p-4 lg:grid-cols-[15rem_1fr]')],
            [
              h.aside(
                [
                  Ui.className<Message>(
                    'hidden min-h-[22rem] rounded-[4px] border border-white/8 bg-white/[0.025] p-3 lg:block',
                  ),
                ],
                [
                  h.div(
                    [Ui.className<Message>('mb-5 h-7 rounded-[4px] bg-white/[0.06]')],
                    [],
                  ),
                  ...['Overview', 'Customers', 'Billing', 'Settings'].map(item =>
                    h.div(
                      [
                        Ui.className<Message>(
                          'mb-2 h-8 rounded-[4px] border border-white/6 bg-white/[0.03] px-3 py-2 text-[0.68rem] text-white/45',
                        ),
                      ],
                      [item],
                    ),
                  ),
                ],
              ),
              h.section(
                [Ui.className<Message>('grid min-w-0 gap-4')],
                [
                  h.div(
                    [Ui.className<Message>('grid gap-3 sm:grid-cols-3')],
                    [
                      dashboardMetric<Message>('Revenue', '$82.4K'),
                      dashboardMetric<Message>('Users', '34.7K'),
                      dashboardMetric<Message>('Growth', '+28%'),
                    ],
                  ),
                  h.img([
                    h.Src('/dashboard-dark.png'),
                    h.Alt('Launch UI dashboard screenshot'),
                    Ui.className<Message>(
                      'block aspect-[134/82] w-full rounded-[4px] border border-white/8 object-cover object-top opacity-95',
                    ),
                  ]),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

export const view = <Message,>(): Html => {
  const h = html<Message>()

  return h.main(
    [
      h.DataAttribute('route', 'new-landing'),
      h.DataAttribute('launch-ui-replica', 'blue-minimal'),
      Ui.className<Message>(
        'relative min-h-dvh overflow-hidden bg-[#02040a] text-white',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_50%_34%,rgba(37,99,235,0.32),transparent_34%),linear-gradient(180deg,rgba(255,255,255,0.04),transparent_26%)]',
          ),
        ],
        [],
      ),
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.035)_1px,transparent_1px)] bg-[size:72px_72px] opacity-25',
          ),
        ],
        [],
      ),
      h.header(
        [
          Ui.className<Message>(
            'relative z-10 mx-auto flex w-full max-w-7xl items-center justify-between gap-4 px-5 py-4 sm:px-7',
          ),
        ],
        [
          h.a(
            [
              h.Href('/new'),
              Ui.className<Message>('flex items-center gap-3 no-underline'),
            ],
            [
              logoMark<Message>(),
              h.span([Ui.className<Message>('font-semibold text-white')], [
                'OpenAgents',
              ]),
              h.span(
                [
                  Ui.className<Message>(
                    'hidden rounded-[4px] border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[0.62rem] text-white/45 sm:inline-flex',
                  ),
                ],
                ['v2'],
              ),
            ],
          ),
          h.nav(
            [
              h.AriaLabel('Primary'),
              Ui.className<Message>(
                'hidden items-center gap-5 text-[0.78rem] text-white/54 lg:flex',
              ),
            ],
            navItems.map(item =>
              h.a(
                [h.Href('#'), Ui.className<Message>('hover:text-white')],
                [item],
              ),
            ),
          ),
          h.div(
            [Ui.className<Message>('flex items-center gap-2')],
            [
              iconButton<Message>('Open source', 'G'),
              iconButton<Message>('Search', '/'),
            ],
          ),
        ],
      ),
      h.section(
        [
          Ui.className<Message>(
            'relative z-10 mx-auto flex min-h-[calc(100dvh-4.5rem)] w-full max-w-7xl flex-col px-5 pb-8 pt-8 sm:px-7 sm:pt-14',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('max-w-5xl')],
            [
              h.a(
                [
                  h.Href('#'),
                  Ui.className<Message>(
                    'inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/[0.035] px-3 py-1.5 text-[0.78rem] text-white/68 no-underline backdrop-blur hover:border-white/20 hover:text-white',
                  ),
                ],
                [
                  h.span([Ui.className<Message>('text-white')], [
                    'Launch UI v2 is out!',
                  ]),
                  h.span([], ['Read more ->']),
                ],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 mt-7 max-w-5xl text-balance text-[3.35rem] font-semibold leading-[0.95] text-transparent bg-clip-text bg-gradient-to-b from-white via-white to-white/58 sm:text-[5rem] lg:text-[6.6rem]',
                  ),
                ],
                ['Give your big idea the design it deserves'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 mt-6 max-w-2xl text-pretty text-base leading-7 text-white/54 sm:text-lg',
                  ),
                ],
                [
                  'Beautiful, production-ready components for ambitious builders. Launch with a polished dark interface, sharp sections, and a dashboard hero that feels alive.',
                ],
              ),
              h.div(
                [Ui.className<Message>('mt-7 flex flex-wrap items-center gap-3')],
                [
                  h.a(
                    [
                      h.Href('/business'),
                      Ui.className<Message>(
                        'inline-flex min-h-11 items-center justify-center rounded-[5px] bg-white px-5 text-sm font-semibold text-black no-underline hover:bg-white/88',
                      ),
                    ],
                    ['Get Started'],
                  ),
                  h.a(
                    [
                      h.Href('/docs'),
                      Ui.className<Message>(
                        'inline-flex min-h-11 items-center justify-center gap-2 rounded-[5px] border border-white/12 bg-white/[0.03] px-5 text-sm font-semibold text-white/78 no-underline hover:border-white/22 hover:text-white',
                      ),
                    ],
                    [h.span([], ['G']), h.span([], ['Github'])],
                  ),
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'mt-7 flex flex-wrap items-center gap-3 text-sm text-white/48',
                  ),
                ],
                [
                  h.div(
                    [Ui.className<Message>('flex -space-x-2')],
                    [
                      avatar<Message>('Ava', 'bg-[#2563eb]'),
                      avatar<Message>('Ben', 'bg-[#7c3aed]'),
                      avatar<Message>('Cam', 'bg-[#0891b2]'),
                    ],
                  ),
                  h.span([Ui.className<Message>('text-[#f8d26a]')], ['5.0']),
                  h.span([], ['Used by 34.7k+ companies and builders']),
                ],
              ),
            ],
          ),
          dashboardMockup<Message>(),
        ],
      ),
    ],
  )
}
