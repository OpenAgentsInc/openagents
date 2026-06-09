import { clsx } from 'clsx'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { descriptionList, mediaRows, ratingStars } from './data-display'
import { container, section } from './layout'
import { headingBlock, linkButton } from './shared'
import {
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceClass,
  titleClass,
} from './primitives'
import type {
  CommerceReview,
  DescriptionItem,
  MarketingFaq,
  MarketingFeature,
  MarketingPerson,
  MarketingPost,
  MarketingPricingTier,
  NavItem,
  Tone,
} from './primitives'

export const marketingBanner = <Message>(input: {
  label: string
  action?: Html
  tone?: Tone
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        clsx(
          'grid gap-3 border px-4 py-3 text-sm sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center',
          {
            'border-[#222] bg-[#010102] text-white/60':
              input.tone === undefined || input.tone === 'neutral',
            'border-[#ffb400] bg-[#141414] text-[#ffb400]':
              input.tone === 'accent',
            'border-[#00c853] bg-[#06140b] text-[#00c853]':
              input.tone === 'positive',
            'border-[#ff6f00] bg-[#160a00] text-[#ff6f00]':
              input.tone === 'warning',
            'border-[#d32f2f] bg-[#170505] text-[#d32f2f]':
              input.tone === 'negative',
            'border-[#2979ff] bg-[#050b18] text-[#2979ff]':
              input.tone === 'info',
          },
        ),
      ),
    ],
    [
      h.p([h.Class('m-0 min-w-0 overflow-wrap-anywhere')], [input.label]),
      input.action ?? null,
    ],
  )
}

export const marketingHeader = <Message>(input: {
  brand: string
  nav: ReadonlyArray<NavItem>
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.header(
    [h.Class('border-b border-[#222] bg-[#010102]')],
    [
      container<Message>(
        [
          h.a(
            [
              h.Href('/'),
              h.Class(
                'text-sm font-medium uppercase tracking-[0.08em] text-[#f1efe8] no-underline',
              ),
            ],
            [input.brand],
          ),
          h.nav(
            [h.Class('hidden items-center gap-1 md:flex')],
            input.nav.map(item =>
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    clsx(
                      'border border-transparent px-2.5 py-2 text-sm text-white/55 no-underline hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]',
                      {
                        'border-[#333] bg-[#141414] text-[#f1efe8]':
                          item.active === true,
                      },
                    ),
                  ),
                ],
                [item.label],
              ),
            ),
          ),
          input.action ?? null,
        ],
        [
          h.Class(
            'mx-auto grid w-[min(100%,1120px)] grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-4 px-4 py-3',
          ),
        ],
      ),
    ],
  )
}

export const marketingHero = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  primaryAction?: Html
  secondaryAction?: Html
  aside?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.Class(
        'grid min-h-[min(720px,calc(100dvh-4rem))] items-center gap-8 border-x border-[#222] bg-[#000] px-4 py-14 md:grid-cols-[minmax(0,1.05fr)_minmax(280px,0.95fr)]',
      ),
    ],
    [
      h.div(
        [h.Class('min-w-0')],
        [
          headingBlock<Message>({
            eyebrow: input.eyebrow ?? 'OpenAgents',
            title: input.title,
            ...(input.body === undefined ? {} : { body: input.body }),
            level: 1,
          }),
          h.div(
            [h.Class('mt-6 flex flex-wrap items-center gap-3')],
            [input.primaryAction ?? null, input.secondaryAction ?? null],
          ),
        ],
      ),
      input.aside ??
        h.div(
          [
            h.Class(
              'grid min-h-72 place-items-center border border-[#222] bg-[#010102] p-6',
            ),
          ],
          [
            h.div(
              [h.Class('grid w-full max-w-sm gap-2')],
              [
                h.div(
                  [h.Class('h-2 border border-[#ffb400] bg-[#ffb400]/25')],
                  [],
                ),
                h.div(
                  [h.Class('h-2 border border-[#2979ff] bg-[#2979ff]/20')],
                  [],
                ),
                h.div(
                  [h.Class('h-2 border border-[#00c853] bg-[#00c853]/20')],
                  [],
                ),
              ],
            ),
          ],
        ),
    ],
  )
}

export const marketingLandingPage = <Message>(input: {
  banner?: Html
  hero: Html
  logos?: ReadonlyArray<string>
  features: ReadonlyArray<MarketingFeature>
  faq?: ReadonlyArray<MarketingFaq>
  cta?: Html
  footerItems?: ReadonlyArray<NavItem>
}): Html => {
  const h = html<Message>()

  return h.div(
    [kitFamily<Message>('page-examples/landing-pages'), h.Class('grid gap-4')],
    [
      input.banner ?? null,
      input.hero,
      input.logos === undefined ? null : logoCloud<Message>(input.logos),
      bentoGrid<Message>(input.features),
      input.faq === undefined ? null : faqSection<Message>(input.faq),
      input.cta ?? null,
      input.footerItems === undefined
        ? null
        : footer<Message>(input.footerItems),
    ],
  )
}

export const bentoGrid = <Message>(
  features: ReadonlyArray<MarketingFeature>,
): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.Class(
        'grid grid-cols-1 gap-px border border-[#222] bg-[#222] md:grid-cols-3',
      ),
    ],
    features.map((feature, index) =>
      h.article(
        [
          h.Class(
            clsx('grid gap-3 bg-[#010102] p-4', {
              'md:col-span-2': index === 0,
              'md:row-span-2': index === 1,
            }),
          ),
        ],
        [
          h.span([h.Class(statusDotClass(feature.tone ?? 'neutral'))], []),
          h.h3(
            [h.Class('m-0 text-lg font-medium text-white/90')],
            [feature.title],
          ),
          h.p([h.Class('m-0 text-sm leading-6 text-white/55')], [feature.body]),
          feature.meta === undefined
            ? null
            : h.p([h.Class(clsx(metaClass, 'mt-auto'))], [feature.meta]),
        ],
      ),
    ),
  )
}

export const featureSection = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  features: ReadonlyArray<MarketingFeature>
}): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-6 p-4'))],
    [
      headingBlock<Message>({
        eyebrow: input.eyebrow ?? 'Features',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
      }),
      h.div(
        [h.Class('grid gap-px border border-[#222] bg-[#222] md:grid-cols-2')],
        input.features.map(feature =>
          h.article(
            [h.Class('grid gap-2 bg-[#010102] p-4')],
            [
              h.div([h.Class(statusDotClass(feature.tone ?? 'neutral'))], []),
              h.h3([h.Class(titleClass)], [feature.title]),
              h.p(
                [h.Class('m-0 text-sm leading-6 text-white/55')],
                [feature.body],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

export const contentSection = <Message>(input: {
  title: string
  body: string
  aside?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.Class(
        'grid gap-6 border border-[#222] bg-[#010102] p-4 md:grid-cols-[minmax(0,1fr)_minmax(240px,0.75fr)]',
      ),
    ],
    [
      headingBlock<Message>({
        eyebrow: 'Content',
        title: input.title,
        body: input.body,
      }),
      input.aside ??
        h.div([h.Class('min-h-40 border border-[#222] bg-[#000]')], []),
    ],
  )
}

export const ctaSection = <Message>(input: {
  title: string
  body?: string
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.Class(
        'grid gap-4 border border-[#333] bg-[#141414] p-5 md:grid-cols-[minmax(0,1fr)_auto] md:items-center',
      ),
    ],
    [
      headingBlock<Message>({
        eyebrow: 'Action',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
      }),
      input.action ?? null,
    ],
  )
}

export const faqSection = <Message>(
  items: ReadonlyArray<MarketingFaq>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-0'))],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [
          headingBlock<Message>({
            eyebrow: 'FAQ',
            title: 'Questions',
            level: 3,
          }),
        ],
      ),
      ...items.map(item =>
        h.div(
          [h.Class('grid gap-2 border-b border-[#222] p-4 last:border-b-0')],
          [
            h.h3([h.Class(titleClass)], [item.question]),
            h.p(
              [h.Class('m-0 text-sm leading-6 text-white/55')],
              [item.answer],
            ),
          ],
        ),
      ),
    ],
  )
}

export const pricingGrid = <Message>(
  tiers: ReadonlyArray<MarketingPricingTier>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class('grid gap-px border border-[#222] bg-[#222] md:grid-cols-3')],
    tiers.map(tier =>
      h.article(
        [
          h.Class(
            clsx('grid gap-4 bg-[#010102] p-4', {
              'outline outline-1 outline-[#ffb400]': tier.highlighted === true,
            }),
          ),
        ],
        [
          headingBlock<Message>({
            eyebrow: tier.name,
            title: tier.price,
            ...(tier.description === undefined
              ? {}
              : { body: tier.description }),
            level: 3,
          }),
          h.ul(
            [h.Role('list'), h.Class('m-0 grid list-none gap-2 p-0')],
            tier.features.map(feature =>
              h.li(
                [h.Class('flex items-center gap-2 text-sm text-white/60')],
                [
                  h.span([h.Class(statusDotClass('positive'))], []),
                  h.span([], [feature]),
                ],
              ),
            ),
          ),
          tier.actionHref === undefined || tier.actionLabel === undefined
            ? null
            : linkButton<Message>({
                href: tier.actionHref,
                label: tier.actionLabel,
                size: 'sm',
                variant: tier.highlighted === true ? 'primary' : 'secondary',
              }),
        ],
      ),
    ),
  )
}

export const testimonialGrid = <Message>(
  reviews: ReadonlyArray<CommerceReview>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class('grid gap-px border border-[#222] bg-[#222] md:grid-cols-2')],
    reviews.map(review =>
      h.figure(
        [h.Class('grid gap-3 bg-[#010102] p-4')],
        [
          review.rating === undefined
            ? null
            : ratingStars<Message>(review.rating),
          h.blockquote(
            [h.Class('m-0 text-sm leading-6 text-white/65')],
            [review.body],
          ),
          h.figcaption(
            [h.Class('grid gap-0.5')],
            [
              h.span([h.Class(titleClass)], [review.author]),
              review.meta === undefined
                ? null
                : h.span([h.Class(metaClass)], [review.meta]),
            ],
          ),
        ],
      ),
    ),
  )
}

export const teamGrid = <Message>(
  people: ReadonlyArray<MarketingPerson>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-4 p-4'))],
    [
      headingBlock<Message>({ eyebrow: 'Team', title: 'People', level: 3 }),
      mediaRows<Message>(
        people.map(person => ({
          title: person.name,
          detail:
            person.handle === undefined
              ? person.role
              : `${person.role} · ${person.handle}`,
          ...(person.avatarUrl === undefined
            ? {}
            : { avatarUrl: person.avatarUrl }),
          fallback: person.name
            .split(/\s+/)
            .map(part => part[0])
            .join('')
            .slice(0, 2)
            .toUpperCase(),
        })),
      ),
    ],
  )
}

export const logoCloud = <Message>(labels: ReadonlyArray<string>): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'grid grid-cols-2 gap-px border border-[#222] bg-[#222] sm:grid-cols-4',
      ),
    ],
    labels.map(label =>
      h.div(
        [
          h.Class(
            'grid min-h-16 place-items-center bg-[#010102] px-3 text-center text-xs uppercase tracking-[0.08em] text-white/35',
          ),
        ],
        [label],
      ),
    ),
  )
}

export const blogList = <Message>(
  posts: ReadonlyArray<MarketingPost>,
): Html => {
  const h = html<Message>()

  return h.section(
    [h.Class(clsx(surfaceClass, 'grid gap-0'))],
    [
      h.div(
        [h.Class('border-b border-[#222] px-4 py-3')],
        [headingBlock<Message>({ eyebrow: 'Log', title: 'Updates', level: 3 })],
      ),
      ...posts.map(post =>
        h.article(
          [h.Class('grid gap-2 border-b border-[#222] p-4 last:border-b-0')],
          [
            h.h3(
              [h.Class(titleClass)],
              [
                post.href === undefined
                  ? post.title
                  : h.a(
                      [
                        h.Href(post.href),
                        h.Class(
                          'text-white/90 no-underline hover:text-[#ffb400]',
                        ),
                      ],
                      [post.title],
                    ),
              ],
            ),
            h.p(
              [h.Class('m-0 text-sm leading-6 text-white/55')],
              [post.excerpt],
            ),
            post.meta === undefined
              ? null
              : h.p([h.Class(metaClass)], [post.meta]),
          ],
        ),
      ),
    ],
  )
}

export const contactSection = <Message>(
  items: ReadonlyArray<DescriptionItem>,
): Html =>
  section<Message>([
    headingBlock<Message>({
      eyebrow: 'Contact',
      title: 'Reach the workspace',
      level: 3,
    }),
    descriptionList<Message>(items),
  ])

export const newsletterSection = <Message>(input: {
  title: string
  body?: string
  action?: Html
}): Html =>
  ctaSection<Message>({
    title: input.title,
    ...(input.body === undefined ? {} : { body: input.body }),
    ...(input.action === undefined ? {} : { action: input.action }),
  })

export const footer = <Message>(items: ReadonlyArray<NavItem>): Html => {
  const h = html<Message>()

  return h.footer(
    [h.Class('border-t border-[#222] bg-[#010102]')],
    [
      container<Message>(
        [
          h.p([h.Class(metaClass)], ['OpenAgents']),
          h.nav(
            [h.Class('flex flex-wrap gap-2')],
            items.map(item =>
              h.a(
                [
                  h.Href(item.href),
                  h.Class(
                    'text-sm text-white/45 no-underline hover:text-[#f1efe8]',
                  ),
                ],
                [item.label],
              ),
            ),
          ),
        ],
        [
          h.Class(
            'mx-auto flex w-[min(100%,1120px)] items-center justify-between gap-4 px-4 py-6',
          ),
        ],
      ),
    ],
  )
}
