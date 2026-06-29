import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { inputClass } from './forms'
import { avatar, headingBlock } from './shared'
import {
  eyebrowClass,
  kitFamily,
  metaClass,
  motionTextRevealAnimationClass,
  rowClass,
  statusDotClass,
  surfaceClass,
  titleClass,
  toneTextClass,
} from './primitives'
import type {
  BadgeItem,
  CalendarDay,
  CommerceFilterGroup,
  CommerceIncentive,
  CommerceLineItem,
  CommerceProductItem,
  CommerceReview,
  CommerceSummaryLine,
  DescriptionItem,
  FeedItem,
  GridListItem,
  KeyValueItem,
  MediaRowItem,
  StackedListItem,
  StatItem,
  TableColumn,
  TableRow,
  Tone,
} from './primitives'

export const keyValueRows = <Message>(
  rows: ReadonlyArray<KeyValueItem>,
): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class('grid gap-0 border-b border-[#222] px-3 py-2.5')],
    rows.map(row =>
      h.div(
        [
          h.Class(
            clsx(
              rowClass,
              'justify-between py-[7px] [&>span:first-child]:overflow-hidden [&>span:first-child]:text-ellipsis [&>span:first-child]:whitespace-nowrap [&>span:first-child]:text-[0.75rem] [&>span:first-child]:text-white/35 [&>span:last-child]:max-w-[55%] [&>span:last-child]:break-words [&>span:last-child]:text-right [&>span:last-child]:text-[0.8125rem] [&>span:last-child]:text-[#f1efe8] [&>a]:max-w-[55%] [&>a]:break-words [&>a]:text-right [&>a]:text-[#f1efe8] [&>a]:underline [&>a]:underline-offset-[3px]',
            ),
          ),
        ],
        [
          h.span([], [row.label]),
          h.span([h.Class(motionTextRevealAnimationClass)], [row.value]),
        ],
      ),
    ),
  )
}

export const codeBlock = <Message>(input: {
  lines: ReadonlyArray<string>
  maxHeightClass?: string
}): Html => {
  const h = html<Message>()

  return h.pre(
    [
      h.Class(
        clsx(
          'm-0 grid overflow-auto px-3 py-2.5 font-[inherit] text-[0.75rem] leading-[1.55] text-white/60 [&_code]:block [&_code]:whitespace-pre',
          input.maxHeightClass ?? 'max-h-[220px]',
        ),
      ),
    ],
    input.lines.map((line, index) =>
      h.code([], [`${String(index + 1).padStart(2, '0')}  ${line}`]),
    ),
  )
}

export const badge = <Message>(input: BadgeItem): Html => {
  const h = html<Message>()

  return h.span(
    [
      kitFamily<Message>('elements/badges'),
      h.Class(
        clsx(
          'inline-flex min-h-6 max-w-full items-center gap-1 border px-2 text-[0.6875rem] uppercase leading-none tracking-[0.08em]',
          {
            'border-white/15 text-white/45':
              input.tone === undefined || input.tone === 'neutral',
            'border-[#ffb400]/70 text-[#ffb400]': input.tone === 'accent',
            'border-[#00c853]/70 text-[#00c853]': input.tone === 'positive',
            'border-[#ff6f00]/70 text-[#ff6f00]': input.tone === 'warning',
            'border-[#d32f2f]/70 text-[#d32f2f]': input.tone === 'negative',
            'border-[#2979ff]/70 text-[#2979ff]': input.tone === 'info',
          },
        ),
      ),
    ],
    [
      h.span([h.Class(statusDotClass(input.tone ?? 'neutral'))], []),
      h.span(
        [h.Class('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap')],
        [input.label],
      ),
    ],
  )
}

export const mediaObject = <Message>(input: {
  title: string
  body?: string
  avatarUrl?: string
  meta?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('layout/media-objects'),
      h.Class('grid grid-cols-[auto_minmax(0,1fr)] gap-3'),
    ],
    [
      avatar<Message>({
        name: input.title,
        ...(input.avatarUrl === undefined ? {} : { imageUrl: input.avatarUrl }),
        size: 'md',
      }),
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class(titleClass)], [input.title]),
          input.body === undefined
            ? null
            : h.p(
                [h.Class('m-0 text-sm leading-6 text-white/55')],
                [input.body],
              ),
          input.meta === undefined
            ? null
            : h.p([h.Class(metaClass)], [input.meta]),
        ],
      ),
    ],
  )
}

export const statGrid = <Message>(
  stats: ReadonlyArray<StatItem>,
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      h.Class(
        'grid grid-cols-1 gap-px border border-[#222] bg-[#222] sm:grid-cols-3',
      ),
    ],
    stats.map(stat =>
      h.div(
        [h.Class('min-w-0 bg-[#010102] p-4')],
        [
          h.p([h.Class(clsx(eyebrowClass, 'mb-2'))], [stat.label]),
          h.p(
            [
              h.Class(
                clsx(
                  'm-0 text-3xl font-semibold leading-none',
                  toneTextClass(stat.tone ?? 'neutral'),
                ),
              ),
            ],
            [stat.value],
          ),
        ],
      ),
    ),
  )
}

export const calendarMonth = <Message>(input: {
  title: string
  days: ReadonlyArray<CalendarDay>
}): Html => {
  const h = html<Message>()
  const weekdays = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  return h.section(
    [kitFamily<Message>('data-display/calendars'), h.Class(surfaceClass)],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [
          headingBlock<Message>({
            eyebrow: 'Calendar',
            title: input.title,
            level: 3,
          }),
        ],
      ),
      h.div(
        [h.Class('grid grid-cols-7 border-b border-[#222] text-center')],
        weekdays.map(day =>
          h.span(
            [
              h.Class(
                'border-r border-[#222] py-2 text-[0.6875rem] uppercase text-white/35 last:border-r-0',
              ),
            ],
            [day],
          ),
        ),
      ),
      h.div(
        [h.Class('grid grid-cols-7')],
        input.days.map(day =>
          h.div(
            [
              h.Class(
                clsx(
                  'grid min-h-20 content-start gap-2 border-b border-r border-[#222] p-2 last:border-r-0',
                  {
                    'bg-[#141414] outline outline-1 outline-[#ffb400]':
                      day.active === true,
                  },
                ),
              ),
            ],
            [
              h.span([h.Class('text-xs text-white/50')], [day.label]),
              day.meta === undefined
                ? null
                : h.span(
                    [
                      h.Class(
                        clsx('text-[0.6875rem]', toneTextClass(day.tone)),
                      ),
                    ],
                    [day.meta],
                  ),
            ],
          ),
        ),
      ),
    ],
  )
}

export const descriptionList = <Message>(
  items: ReadonlyArray<DescriptionItem>,
): Html => {
  const h = html<Message>()

  return h.dl(
    [h.Class('grid border-t border-[#222]')],
    items.map(item =>
      h.div(
        [
          h.Class(
            'grid grid-cols-[minmax(7rem,0.45fr)_minmax(0,1fr)] gap-4 border-b border-[#222] py-3 text-sm',
          ),
        ],
        [
          h.dt([h.Class('min-w-0 text-white/35')], [item.label]),
          h.dd(
            [
              h.Class(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80',
              ),
            ],
            [item.value],
          ),
        ],
      ),
    ),
  )
}

export const stackedList = <Message>(
  items: ReadonlyArray<StackedListItem>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [h.Role('list'), h.Class('m-0 grid list-none p-0')],
    items.map(item =>
      h.li(
        [
          h.Class(
            'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border-b border-[#222] px-3 py-2.5 last:border-b-0',
          ),
        ],
        [
          h.span([h.Class(statusDotClass(item.tone ?? 'neutral'))], []),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(titleClass)], [item.title]),
              item.detail === undefined
                ? null
                : h.p([h.Class(metaClass)], [item.detail]),
            ],
          ),
          item.meta === undefined
            ? null
            : h.em([h.Class('text-xs not-italic text-white/35')], [item.meta]),
        ],
      ),
    ),
  )
}

export const mediaRows = <Message>(
  items: ReadonlyArray<MediaRowItem>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [h.Role('list'), h.Class('m-0 grid list-none gap-2 p-0')],
    items.map(item =>
      h.li(
        [
          h.Class(
            'grid min-w-0 grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3',
          ),
        ],
        [
          item.avatarUrl === undefined
            ? h.div(
                [
                  h.Class(
                    'grid h-10 w-10 place-items-center border border-[#222] bg-[#010102] text-xs text-white/45',
                  ),
                ],
                [item.fallback ?? 'OA'],
              )
            : h.img([
                h.Src(item.avatarUrl),
                h.Alt(''),
                h.Class('h-10 w-10 border border-[#222] object-cover'),
              ]),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(titleClass)], [item.title]),
              item.detail === undefined
                ? null
                : h.p([h.Class(metaClass)], [item.detail]),
            ],
          ),
          item.meta === undefined
            ? null
            : h.span(
                [
                  h.Class(
                    'border border-[#222] px-2 py-1 text-[0.6875rem] uppercase leading-none text-white/45',
                  ),
                ],
                [item.meta],
              ),
        ],
      ),
    ),
  )
}

export const feedList = <Message>(items: ReadonlyArray<FeedItem>): Html => {
  const h = html<Message>()

  return h.ol(
    [
      kitFamily<Message>('lists/feeds'),
      h.Class('m-0 grid list-none border-l border-[#222] p-0'),
    ],
    items.map(item =>
      h.li(
        [h.Class('relative grid gap-1 border-b border-[#222] py-3 pl-4')],
        [
          h.span(
            [
              h.Class(
                clsx(
                  'absolute -left-[5px] top-4',
                  statusDotClass(item.tone ?? 'neutral'),
                ),
              ),
            ],
            [],
          ),
          h.p([h.Class(titleClass)], [item.title]),
          item.body === undefined
            ? null
            : h.p(
                [h.Class('m-0 text-sm leading-6 text-white/55')],
                [item.body],
              ),
          item.meta === undefined
            ? null
            : h.p([h.Class(metaClass)], [item.meta]),
        ],
      ),
    ),
  )
}

export const gridList = <Message>(items: ReadonlyArray<GridListItem>): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('lists/grid-lists'),
      h.Class('grid gap-px border border-[#222] bg-[#222] sm:grid-cols-2'),
    ],
    items.map(item => {
      const content = [
        h.span([h.Class(statusDotClass(item.tone ?? 'neutral'))], []),
        h.h3(
          [h.Class('m-0 text-base font-medium text-white/90')],
          [item.title],
        ),
        item.body === undefined
          ? null
          : h.p([h.Class('m-0 text-sm leading-6 text-white/55')], [item.body]),
        item.meta === undefined ? null : h.p([h.Class(metaClass)], [item.meta]),
      ]

      if (item.href !== undefined) {
        return h.a(
          [
            h.Href(item.href),
            h.Class(
              'grid gap-3 bg-[#010102] p-4 text-inherit no-underline hover:bg-[#080808]',
            ),
          ],
          content,
        )
      }

      return h.article([h.Class('grid gap-3 bg-[#010102] p-4')], content)
    }),
  )
}

export const listContainer = <Message>(input: {
  title: string
  items: ReadonlyArray<StackedListItem>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('layout/list-containers'),
      h.Class(clsx(surfaceClass, 'grid gap-0')),
    ],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [
          headingBlock<Message>({
            eyebrow: 'List',
            title: input.title,
            level: 3,
          }),
        ],
      ),
      stackedList<Message>(input.items),
    ],
  )
}

const swatchClass = (tone: Tone): string =>
  clsx('h-4 w-4 border', {
    'border-white/30 bg-white/30': tone === 'neutral',
    'border-[#ffb400] bg-[#ffb400]/30': tone === 'accent',
    'border-[#00c853] bg-[#00c853]/25': tone === 'positive',
    'border-[#ff6f00] bg-[#ff6f00]/25': tone === 'warning',
    'border-[#d32f2f] bg-[#d32f2f]/25': tone === 'negative',
    'border-[#2979ff] bg-[#2979ff]/25': tone === 'info',
  })

export const ratingStars = <Message>(rating: number): Html => {
  const h = html<Message>()
  const safeRating = Math.max(0, Math.min(5, Math.round(rating)))

  return h.div(
    [h.Class('flex items-center gap-1'), h.AriaLabel(`${safeRating} of 5`)],
    Array.from({ length: 5 }, (_, index) =>
      h.span(
        [
          h.Class(
            clsx('h-2 w-2 border', {
              'border-[#ffb400] bg-[#ffb400]': index < safeRating,
              'border-white/20 bg-transparent': index >= safeRating,
            }),
          ),
        ],
        [],
      ),
    ),
  )
}

export const productGrid = <Message>(input: {
  title?: string
  products: ReadonlyArray<CommerceProductItem>
  columns?: 2 | 3 | 4
}): Html => {
  const h = html<Message>()
  const columns = input.columns ?? 3

  return h.section(
    [h.Class(surfaceClass)],
    [
      input.title === undefined
        ? null
        : h.div(
            [h.Class('border-b border-[#222] px-4 py-3')],
            [
              headingBlock<Message>({
                eyebrow: 'Catalog',
                title: input.title,
                level: 3,
              }),
            ],
          ),
      h.div(
        [
          h.Class(
            clsx('grid border-l border-[#222]', {
              'grid-cols-1 sm:grid-cols-2': columns === 2,
              'grid-cols-1 sm:grid-cols-2 lg:grid-cols-3': columns === 3,
              'grid-cols-1 sm:grid-cols-2 lg:grid-cols-4': columns === 4,
            }),
          ),
        ],
        input.products.map(product =>
          h.article(
            [
              h.Class(
                'group relative min-w-0 border-b border-r border-[#222] bg-[#010102] p-4',
              ),
            ],
            [
              product.imageUrl === undefined
                ? h.div(
                    [
                      h.Class(
                        'grid aspect-square place-items-center border border-[#222] bg-[#000] text-xs uppercase text-white/25',
                      ),
                    ],
                    ['OpenAgents'],
                  )
                : h.img([
                    h.Src(product.imageUrl),
                    h.Alt(product.imageAlt ?? ''),
                    h.Class(
                      'aspect-square w-full border border-[#222] bg-[#000] object-cover opacity-90 group-hover:opacity-100',
                    ),
                  ]),
              h.div(
                [h.Class('grid gap-3 pt-4')],
                [
                  h.h3(
                    [h.Class('m-0 text-sm font-medium text-white/90')],
                    [
                      product.href === undefined
                        ? product.title
                        : h.a(
                            [
                              h.Href(product.href),
                              h.Class(
                                'text-white/90 no-underline hover:text-[#ffb400]',
                              ),
                            ],
                            [product.title],
                          ),
                    ],
                  ),
                  product.detail === undefined
                    ? null
                    : h.p([h.Class(metaClass)], [product.detail]),
                  product.rating === undefined
                    ? null
                    : h.div(
                        [h.Class('flex items-center justify-between gap-3')],
                        [
                          ratingStars<Message>(product.rating),
                          product.reviewCount === undefined
                            ? null
                            : h.span(
                                [h.Class(metaClass)],
                                [`${product.reviewCount} reviews`],
                              ),
                        ],
                      ),
                  product.swatches === undefined
                    ? null
                    : h.div(
                        [h.Class('flex flex-wrap gap-1.5')],
                        product.swatches.map(tone =>
                          h.span([h.Class(swatchClass(tone))], []),
                        ),
                      ),
                  product.price === undefined
                    ? null
                    : h.p(
                        [h.Class('m-0 text-sm font-medium text-[#f1efe8]')],
                        [product.price],
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

export const commerceLineList = <Message>(
  items: ReadonlyArray<CommerceLineItem>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [h.Role('list'), h.Class('m-0 grid list-none border-y border-[#222] p-0')],
    items.map(item =>
      h.li(
        [
          h.Class(
            'grid min-w-0 grid-cols-[56px_minmax(0,1fr)_auto] gap-3 border-b border-[#222] py-3 last:border-b-0',
          ),
        ],
        [
          item.imageUrl === undefined
            ? h.div(
                [
                  h.Class(
                    'grid h-14 w-14 place-items-center border border-[#222] bg-[#010102] text-[0.625rem] uppercase text-white/25',
                  ),
                ],
                ['OA'],
              )
            : h.img([
                h.Src(item.imageUrl),
                h.Alt(item.imageAlt ?? ''),
                h.Class('h-14 w-14 border border-[#222] object-cover'),
              ]),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(titleClass)], [item.title]),
              item.detail === undefined
                ? null
                : h.p([h.Class(metaClass)], [item.detail]),
              item.status === undefined
                ? null
                : h.p(
                    [
                      h.Class(
                        clsx(
                          'm-0 mt-2 text-xs',
                          toneTextClass(item.tone ?? 'neutral'),
                        ),
                      ),
                    ],
                    [item.status],
                  ),
            ],
          ),
          h.div(
            [h.Class('grid justify-items-end gap-1 text-right')],
            [
              item.price === undefined
                ? null
                : h.span([h.Class('text-sm text-white/90')], [item.price]),
              item.quantity === undefined
                ? null
                : h.span([h.Class(metaClass)], [`qty ${item.quantity}`]),
            ],
          ),
        ],
      ),
    ),
  )
}

export const orderSummary = <Message>(input: {
  title?: string
  lines: ReadonlyArray<CommerceSummaryLine>
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-4 p-4'))],
    [
      input.title === undefined
        ? null
        : headingBlock<Message>({
            eyebrow: 'Summary',
            title: input.title,
            level: 3,
          }),
      h.dl(
        [h.Class('grid border-y border-[#222]')],
        input.lines.map(line =>
          h.div(
            [
              h.Class(
                'flex min-w-0 items-center justify-between gap-4 border-b border-[#222] py-3 last:border-b-0',
              ),
            ],
            [
              h.dt(
                [
                  h.Class(
                    clsx('min-w-0 text-sm', {
                      'font-medium text-white/90': line.strong === true,
                      'text-white/45': line.strong !== true,
                    }),
                  ),
                ],
                [line.label],
              ),
              h.dd(
                [
                  h.Class(
                    clsx(
                      'm-0 shrink-0 text-sm',
                      line.strong === true
                        ? 'font-medium text-white/90'
                        : toneTextClass(line.tone ?? 'neutral'),
                    ),
                  ),
                ],
                [line.value],
              ),
            ],
          ),
        ),
      ),
      input.action ?? null,
    ],
  )
}

export const filterPanel = <Message>(
  groups: ReadonlyArray<CommerceFilterGroup>,
): Html => {
  const h = html<Message>()

  return h.aside(
    [h.Class(clsx(surfaceClass, 'grid gap-0'))],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [
          headingBlock<Message>({
            eyebrow: 'Filters',
            title: 'Refine',
            level: 3,
          }),
        ],
      ),
      ...groups.map(group =>
        h.fieldset(
          [h.Class('border-0 border-b border-[#222] p-4 last:border-b-0')],
          [
            h.legend([h.Class(clsx(eyebrowClass, 'mb-3'))], [group.label]),
            h.div(
              [h.Class('grid gap-3')],
              group.options.map((option, index) =>
                h.label(
                  [
                    h.Class(
                      'flex min-w-0 items-center gap-3 text-sm text-white/60',
                    ),
                  ],
                  [
                    h.input([
                      h.Type('checkbox'),
                      h.Name(group.label),
                      h.Value(option.label),
                      h.Class(
                        'h-4 w-4 appearance-none border border-[#333] bg-[#000] checked:border-[#ffb400] checked:bg-[#ffb400] focus:outline-none focus:ring-1 focus:ring-[#ffb400]',
                      ),
                      ...(index === 0 ? [h.Checked(true)] : []),
                    ]),
                    h.span(
                      [
                        h.Class(
                          'min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap',
                        ),
                      ],
                      [option.label],
                    ),
                    option.count === undefined
                      ? null
                      : h.em(
                          [h.Class('not-italic text-white/35')],
                          [option.count],
                        ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    ],
  )
}

export const checkoutForm = <Message>(input: {
  title: string
  fields: ReadonlyArray<DescriptionItem>
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.form(
    [h.Class(clsx(surfaceClass, 'grid gap-4 p-4'))],
    [
      headingBlock<Message>({
        eyebrow: 'Checkout',
        title: input.title,
        level: 3,
      }),
      h.div(
        [h.Class('grid gap-3 sm:grid-cols-2')],
        input.fields.map(field =>
          h.label(
            [h.Class('grid gap-1.5')],
            [
              h.span([h.Class(eyebrowClass)], [field.label]),
              h.input([
                h.Type('text'),
                h.Value(field.value),
                h.Class(inputClass),
              ]),
            ],
          ),
        ),
      ),
      input.action ?? null,
    ],
  )
}

export const reviewList = <Message>(
  reviews: ReadonlyArray<CommerceReview>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-0'))],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [
          headingBlock<Message>({
            eyebrow: 'Reviews',
            title: 'Signal',
            level: 3,
          }),
        ],
      ),
      ...reviews.map(review =>
        h.article(
          [h.Class('grid gap-3 border-b border-[#222] p-4 last:border-b-0')],
          [
            h.div(
              [h.Class('flex items-center justify-between gap-3')],
              [
                h.div(
                  [h.Class('min-w-0')],
                  [
                    h.h3([h.Class(titleClass)], [review.author]),
                    review.meta === undefined
                      ? null
                      : h.p([h.Class(metaClass)], [review.meta]),
                  ],
                ),
                review.rating === undefined
                  ? null
                  : ratingStars<Message>(review.rating),
              ],
            ),
            h.p(
              [h.Class('m-0 text-sm leading-6 text-white/60')],
              [review.body],
            ),
          ],
        ),
      ),
    ],
  )
}

export const incentiveGrid = <Message>(
  incentives: ReadonlyArray<CommerceIncentive>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'grid grid-cols-1 gap-px border border-[#222] bg-[#222] md:grid-cols-3',
      ),
    ],
    incentives.map(incentive =>
      h.div(
        [h.Class('grid gap-3 bg-[#010102] p-4')],
        [
          h.span([h.Class(statusDotClass(incentive.tone ?? 'neutral'))], []),
          h.h3([h.Class(titleClass)], [incentive.title]),
          h.p(
            [h.Class('m-0 text-sm leading-6 text-white/55')],
            [incentive.body],
          ),
        ],
      ),
    ),
  )
}

export const promoBand = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.Class(
        'grid gap-5 border border-[#333] bg-[#141414] p-5 md:grid-cols-[minmax(0,1fr)_auto] md:items-end',
      ),
    ],
    [
      headingBlock<Message>({
        eyebrow: input.eyebrow ?? 'Promo',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        level: 2,
      }),
      input.action ?? null,
    ],
  )
}

export const statsTimeline = <Message>(
  stats: ReadonlyArray<StatItem>,
): Html => {
  const h = html<Message>()

  return h.ol(
    [h.Class('m-0 grid list-none border-l border-[#222] p-0')],
    stats.map(stat =>
      h.li(
        [h.Class('relative grid gap-1 border-b border-[#222] py-3 pl-4')],
        [
          h.span(
            [
              h.Class(
                clsx(
                  'absolute -left-[5px] top-4',
                  statusDotClass(stat.tone ?? 'neutral'),
                ),
              ),
            ],
            [],
          ),
          h.span([h.Class(eyebrowClass)], [stat.label]),
          h.strong(
            [h.Class(clsx('text-xl font-medium', toneTextClass(stat.tone)))],
            [stat.value],
          ),
        ],
      ),
    ),
  )
}

export const tableList = <Message>(input: {
  columns: ReadonlyArray<TableColumn>
  rows: ReadonlyArray<TableRow>
  caption?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('lists/tables'),
      h.Class('-mx-4 -my-2 overflow-x-auto whitespace-nowrap'),
    ],
    [
      h.div(
        [h.Class('inline-block min-w-full px-4 py-2 align-middle')],
        [
          h.table(
            [h.Class('w-full border-collapse text-left text-sm')],
            [
              input.caption === undefined
                ? null
                : h.caption([h.Class('sr-only')], [input.caption]),
              h.thead(
                [h.Class('border-b border-white/10 text-white/45')],
                [
                  h.tr(
                    [],
                    input.columns.map(column =>
                      h.th(
                        [
                          h.Scope('col'),
                          h.Class(
                            clsx(
                              'whitespace-nowrap px-0 py-2 pr-4 font-medium normal-case',
                              {
                                'text-right': column.align === 'right',
                                'text-left': column.align !== 'right',
                              },
                            ),
                          ),
                        ],
                        [column.label],
                      ),
                    ),
                  ),
                ],
              ),
              h.tbody(
                [h.Class('divide-y divide-white/10')],
                input.rows.map(row =>
                  h.tr(
                    [],
                    input.columns.map(column =>
                      h.td(
                        [
                          h.Class(
                            clsx('px-0 py-3 pr-4 text-white/65', {
                              'text-right': column.align === 'right',
                              'text-left': column.align !== 'right',
                            }),
                          ),
                        ],
                        [row.cells[column.key] ?? ''],
                      ),
                    ),
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
