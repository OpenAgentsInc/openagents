import { Effect } from 'effect'
import React, { type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'

import { methodNotAllowed } from './http/responses'

type LandingVariant = 'demo' | 'new'

const h = React.createElement

const launchUiNavItems = [
  'Docs',
  'Components',
  'Blocks',
  'Illustrations',
  'Templates',
  'Pricing',
]

const openAgentsNavItems = [
  'Khala Code',
  'Desktop',
  'Business',
  'Reactor',
  'Receipts',
  'Docs',
]

const productSurfaces = [
  {
    name: 'Khala Code mobile',
    status: 'Mobile entry',
    detail: 'Phone-first coding agent with hosted Agent Computers.',
  },
  {
    name: 'Khala Code desktop',
    status: 'Operator console',
    detail: 'Fleet console, approvals inbox, and workspace review.',
  },
  {
    name: 'openagents.com',
    status: 'Counting house',
    detail: 'Business account, receipts, spend, promises, and APIs.',
  },
  {
    name: 'Reactor',
    status: 'Sovereignty lane',
    detail: 'Private open-model lane governed by typed model policy.',
  },
]

const logoMark = (): ReactNode =>
  h(
    'div',
    {
      className:
        'grid size-8 place-items-center rounded-[4px] border border-white/15 bg-white/[0.06] text-sm font-semibold text-white shadow-[0_0_28px_rgba(37,99,235,0.34)]',
    },
    'OA',
  )

const iconButton = (label: string, glyph: string): ReactNode =>
  h(
    'a',
    {
      'aria-label': label,
      className:
        'grid size-9 place-items-center rounded-[4px] border border-white/10 bg-white/[0.03] text-sm text-white/58 transition hover:border-white/20 hover:bg-white/[0.06] hover:text-white',
      href: '#',
    },
    glyph,
  )

const avatar = (label: string, color: string): ReactNode =>
  h(
    'span',
    {
      'aria-label': label,
      className: `grid size-7 place-items-center rounded-full border-2 border-[#05060a] text-[0.62rem] font-semibold text-white ${color}`,
    },
    label.slice(0, 1),
  )

const dashboardMetric = (label: string, value: string): ReactNode =>
  h(
    'div',
    {
      className: 'rounded-[4px] border border-white/8 bg-white/[0.035] p-3',
    },
    h('p', { className: 'm-0 text-[0.62rem] text-white/38' }, label),
    h('p', { className: 'm-0 mt-2 text-xl font-semibold text-white' }, value),
  )

const shell = (
  variant: LandingVariant,
  title: string,
  description: string,
  body: ReactNode,
): string => `<!doctype html>${renderToStaticMarkup(
  h(
    'html',
    { lang: 'en' },
    h(
      'head',
      null,
      h('meta', { charSet: 'utf-8' }),
      h('meta', {
        content: 'width=device-width, initial-scale=1',
        name: 'viewport',
      }),
      h('title', null, title),
      h('meta', { content: description, name: 'description' }),
      h('link', { href: '/favicon.svg', rel: 'icon', type: 'image/svg+xml' }),
      h('link', { href: '/assets/openagents.css', rel: 'stylesheet' }),
    ),
    h(
      'body',
      {
        className: 'm-0 bg-[#02040a]',
        'data-react-landing-route': variant,
      },
      body,
    ),
  ),
)}`

const heroShell = (
  variant: LandingVariant,
  badge: ReactNode,
  headline: string,
  description: string,
  primary: ReactNode,
  secondary: ReactNode,
  proof: ReactNode,
  mockup: ReactNode,
  navItems: ReadonlyArray<string>,
): ReactNode =>
  h(
    'main',
    {
      className: 'relative min-h-dvh overflow-hidden bg-[#02040a] text-white',
      'data-route': variant === 'demo' ? 'demo-landing' : 'new-landing',
    },
    h('div', {
      className:
        'pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_50%_34%,rgba(37,99,235,0.28),transparent_34%),linear-gradient(180deg,rgba(255,255,255,0.04),transparent_26%)]',
    }),
    h('div', {
      className:
        'pointer-events-none absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.035)_1px,transparent_1px)] bg-[size:72px_72px] opacity-25',
    }),
    h(
      'header',
      {
        className:
          'relative z-10 mx-auto flex w-full max-w-7xl items-center justify-between gap-4 px-5 py-4 sm:px-7',
      },
      h(
        'a',
        { className: 'flex items-center gap-3 no-underline', href: '/new' },
        logoMark(),
        h('span', { className: 'font-semibold text-white' }, 'OpenAgents'),
        h(
          'span',
          {
            className:
              'hidden rounded-[4px] border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[0.62rem] text-white/45 sm:inline-flex',
          },
          variant === 'demo' ? 'v2' : 'suite',
        ),
      ),
      h(
        'nav',
        {
          'aria-label': 'Primary',
          className:
            'hidden items-center gap-5 text-[0.78rem] text-white/54 lg:flex',
        },
        ...navItems.map(item =>
          h('a', { className: 'hover:text-white', href: '#', key: item }, item),
        ),
      ),
      h('div', { className: 'flex items-center gap-2' }, iconButton('Open source', 'G'), iconButton('Search', '/')),
    ),
    h(
      'section',
      {
        className:
          'relative z-10 mx-auto flex min-h-[calc(100dvh-4.5rem)] w-full max-w-7xl flex-col px-5 pb-8 pt-8 sm:px-7 sm:pt-14',
      },
      h(
        'div',
        { className: 'max-w-5xl' },
        badge,
        h(
          'h1',
          {
            className:
              variant === 'demo'
                ? 'm-0 mt-7 max-w-5xl text-balance bg-gradient-to-b from-white via-white to-white/58 bg-clip-text text-[3.35rem] font-semibold leading-[0.95] text-transparent sm:text-[5rem] lg:text-[6.6rem]'
                : 'm-0 mt-7 max-w-5xl text-balance text-[3.1rem] font-semibold leading-[0.98] text-white sm:text-[4.75rem] lg:text-[6rem]',
          },
          headline,
        ),
        h(
          'p',
          {
            className:
              'm-0 mt-6 max-w-3xl text-pretty text-base leading-7 text-white/62 sm:text-lg',
          },
          description,
        ),
        h(
          'div',
          { className: 'mt-7 flex flex-wrap items-center gap-3' },
          primary,
          secondary,
        ),
        proof,
      ),
      mockup,
    ),
  )

const demoBadge = (): ReactNode =>
  h(
    'a',
    {
      className:
        'inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/[0.035] px-3 py-1.5 text-[0.78rem] text-white/68 no-underline backdrop-blur hover:border-white/20 hover:text-white',
      href: '#',
    },
    h('span', { className: 'text-white' }, 'Launch UI v2 is out!'),
    h('span', null, 'Read more ->'),
  )

const demoMockup = (): ReactNode =>
  h(
    'div',
    { className: 'relative mx-auto mt-10 w-full max-w-6xl px-3 pb-2 sm:mt-14' },
    h('div', {
      className:
        'absolute inset-x-4 bottom-0 top-8 rounded-full bg-[#2563eb]/30 blur-3xl',
    }),
    h(
      'div',
      {
        className:
          'relative overflow-hidden rounded-[6px] border border-white/12 bg-[#05060a] shadow-[0_36px_120px_rgba(37,99,235,0.34),0_18px_60px_rgba(0,0,0,0.75)] [transform:perspective(1100px)_rotateX(13deg)] [transform-origin:top_center]',
      },
      h(
        'div',
        {
          className:
            'flex h-10 items-center gap-2 border-b border-white/10 bg-white/[0.035] px-4',
        },
        h('span', { className: 'size-2 rounded-full bg-[#ff5f57]' }),
        h('span', { className: 'size-2 rounded-full bg-[#febc2e]' }),
        h('span', { className: 'size-2 rounded-full bg-[#28c840]' }),
        h('span', {
          className:
            'ml-3 h-5 w-40 rounded-[4px] border border-white/8 bg-black/35',
        }),
      ),
      h(
        'div',
        { className: 'grid gap-4 p-4 lg:grid-cols-[15rem_1fr]' },
        h(
          'aside',
          {
            className:
              'hidden min-h-[22rem] rounded-[4px] border border-white/8 bg-white/[0.025] p-3 lg:block',
          },
          h('div', { className: 'mb-5 h-7 rounded-[4px] bg-white/[0.06]' }),
          ...['Overview', 'Customers', 'Billing', 'Settings'].map(item =>
            h(
              'div',
              {
                className:
                  'mb-2 h-8 rounded-[4px] border border-white/6 bg-white/[0.03] px-3 py-2 text-[0.68rem] text-white/45',
                key: item,
              },
              item,
            ),
          ),
        ),
        h(
          'section',
          { className: 'grid min-w-0 gap-4' },
          h(
            'div',
            { className: 'grid gap-3 sm:grid-cols-3' },
            dashboardMetric('Revenue', '$82.4K'),
            dashboardMetric('Users', '34.7K'),
            dashboardMetric('Growth', '+28%'),
          ),
          h('img', {
            alt: 'Launch UI dashboard screenshot',
            className:
              'block aspect-[134/82] w-full rounded-[4px] border border-white/8 object-cover object-top opacity-95',
            src: '/dashboard-dark.png',
          }),
        ),
      ),
    ),
  )

const newBadge = (): ReactNode =>
  h(
    'a',
    {
      className:
        'inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/[0.035] px-3 py-1.5 text-[0.78rem] text-white/68 no-underline backdrop-blur hover:border-white/20 hover:text-white',
      href: '/docs/product-promises',
    },
    h('span', { className: 'text-white' }, 'Four products, one receipt spine.'),
    h('span', null, 'Read promises ->'),
  )

const productSurfaceRow = (product: (typeof productSurfaces)[number]): ReactNode =>
  h(
    'div',
    {
      className:
        'grid gap-2 rounded-[4px] border border-white/8 bg-white/[0.035] p-3 sm:grid-cols-[10rem_1fr] sm:items-start',
      key: product.name,
    },
    h(
      'div',
      {
        className:
          'text-[0.68rem] font-semibold uppercase tracking-[0.12em] text-white/42',
      },
      product.status,
    ),
    h(
      'div',
      { className: 'min-w-0' },
      h('p', { className: 'm-0 text-sm font-semibold text-white' }, product.name),
      h(
        'p',
        {
          className:
            'm-0 mt-1 text-base leading-7 text-white/58 sm:text-sm sm:leading-6',
        },
        product.detail,
      ),
    ),
  )

const newMockup = (): ReactNode =>
  h(
    'div',
    { className: 'relative mx-auto mt-10 w-full max-w-6xl px-3 pb-2 sm:mt-14' },
    h('div', {
      className:
        'absolute inset-x-4 bottom-0 top-8 rounded-full bg-[#2563eb]/22 blur-3xl',
    }),
    h(
      'div',
      {
        className:
          'relative overflow-hidden rounded-[6px] border border-white/12 bg-[#05060a] shadow-[0_36px_120px_rgba(37,99,235,0.24),0_18px_60px_rgba(0,0,0,0.75)] [transform:perspective(1100px)_rotateX(11deg)] [transform-origin:top_center]',
      },
      h(
        'div',
        {
          className:
            'flex h-10 items-center gap-2 border-b border-white/10 bg-white/[0.035] px-4',
        },
        h('span', { className: 'size-2 rounded-full bg-[#ff5f57]' }),
        h('span', { className: 'size-2 rounded-full bg-[#febc2e]' }),
        h('span', { className: 'size-2 rounded-full bg-[#28c840]' }),
        h(
          'span',
          {
            className:
              'ml-3 h-5 w-44 rounded-[4px] border border-white/8 bg-black/35 px-2 py-1 text-[0.56rem] text-white/35',
          },
          'openagents.com/suite',
        ),
      ),
      h(
        'div',
        { className: 'grid gap-4 p-4 lg:grid-cols-[15rem_1fr]' },
        h(
          'aside',
          {
            className:
              'hidden min-h-[22rem] rounded-[4px] border border-white/8 bg-white/[0.025] p-3 lg:block',
          },
          h(
            'div',
            {
              className:
                'mb-5 h-7 rounded-[4px] bg-white/[0.06] px-3 py-2 text-[0.62rem] text-white/38',
            },
            'OPENAGENTS',
          ),
          ...['Mobile', 'Desktop', 'Business', 'Reactor'].map(item =>
            h(
              'div',
              {
                className:
                  'mb-2 h-8 rounded-[4px] border border-white/6 bg-white/[0.03] px-3 py-2 text-[0.68rem] text-white/45',
                key: item,
              },
              item,
            ),
          ),
        ),
        h(
          'section',
          { className: 'grid min-w-0 gap-4' },
          h(
            'div',
            { className: 'grid gap-3 sm:grid-cols-3' },
            dashboardMetric('Usage truth', 'Exact'),
            dashboardMetric('Authority', 'Typed'),
            dashboardMetric('Receipts', 'Public-safe'),
          ),
          h('div', { className: 'grid gap-3' }, ...productSurfaces.map(productSurfaceRow)),
        ),
      ),
    ),
  )

export const renderReactLandingHtml = (variant: LandingVariant): string =>
  variant === 'demo'
    ? shell(
        variant,
        'Launch UI Replica',
        'Exact original Launch UI replica preserved for comparison.',
        heroShell(
          variant,
          demoBadge(),
          'Give your big idea the design it deserves',
          'Beautiful, production-ready components for ambitious builders. Launch with a polished dark interface, sharp sections, and a dashboard hero that feels alive.',
          h(
            'a',
            {
              className:
                'inline-flex min-h-11 items-center justify-center rounded-[5px] bg-white px-5 text-sm font-semibold text-black no-underline hover:bg-white/88',
              href: '/business',
            },
            'Get Started',
          ),
          h(
            'a',
            {
              className:
                'inline-flex min-h-11 items-center justify-center gap-2 rounded-[5px] border border-white/12 bg-white/[0.03] px-5 text-sm font-semibold text-white/78 no-underline hover:border-white/22 hover:text-white',
              href: '/docs',
            },
            h('span', null, 'G'),
            h('span', null, 'Github'),
          ),
          h(
            'div',
            { className: 'mt-7 flex flex-wrap items-center gap-3 text-sm text-white/48' },
            h('div', { className: 'flex -space-x-2' }, avatar('Ava', 'bg-[#2563eb]'), avatar('Ben', 'bg-[#7c3aed]'), avatar('Cam', 'bg-[#0891b2]')),
            h('span', { className: 'text-[#f8d26a]' }, '5.0'),
            h('span', null, 'Used by 34.7k+ companies and builders'),
          ),
          demoMockup(),
          launchUiNavItems,
        ),
      )
    : shell(
        variant,
        'OpenAgents Suite',
        'OpenAgents suite preview covering Khala Code mobile, Khala Code desktop, openagents.com, and Reactor.',
        heroShell(
          variant,
          newBadge(),
          'The operating system for agents that work',
          'Khala Code on mobile, Khala Code on desktop, openagents.com for business control, and Reactor for private model sovereignty. One suite, one metered substrate, one public-safe receipt trail.',
          h(
            'a',
            {
              className:
                'inline-flex min-h-11 items-center justify-center rounded-[5px] bg-white px-5 text-sm font-semibold text-black no-underline hover:bg-white/88',
              href: '/business',
            },
            'Open business',
          ),
          h(
            'a',
            {
              className:
                'inline-flex min-h-11 items-center justify-center gap-2 rounded-[5px] border border-white/12 bg-white/[0.03] px-5 text-sm font-semibold text-white/78 no-underline hover:border-white/22 hover:text-white',
              href: '/docs',
            },
            'Docs',
          ),
          h(
            'div',
            { className: 'mt-7 flex flex-wrap items-center gap-3 text-sm text-white/48' },
            h('div', { className: 'flex -space-x-2' }, avatar('Mobile', 'bg-[#2563eb]'), avatar('Desktop', 'bg-[#0891b2]'), avatar('Reactor', 'bg-[#7c3aed]')),
            h('span', { className: 'text-[#f8d26a]' }, 'Exact'),
            h('span', null, 'Counters are projections of receipts, not estimates.'),
          ),
          newMockup(),
          openAgentsNavItems,
        ),
      )

export const handleReactLandingPage = (
  request: Request,
  variant: LandingVariant,
) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return Effect.succeed(methodNotAllowed(['GET', 'HEAD']))
  }

  const html = renderReactLandingHtml(variant)
  return Effect.succeed(
    new Response(request.method === 'HEAD' ? null : html, {
      headers: {
        'cache-control': 'no-store',
        'content-type': 'text/html; charset=utf-8',
      },
    }),
  )
}
