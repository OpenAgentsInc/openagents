import { Effect } from 'effect'
import React, { type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'

import { methodNotAllowed } from './http/responses'

type LandingVariant = 'demo' | 'new'

const h = React.createElement

const launchUiNavItems = [
  ['Docs', '/docs/getting-started/introduction'],
  ['Components', '/docs/components/badge'],
  ['Blocks', '/blocks'],
  ['Illustrations', '/illustrations'],
  ['Templates', '/templates'],
  ['Pricing', '/pricing'],
] as const

const openAgentsNavItems = [
  ['Khala Code', '/code'],
  ['Desktop', '/download'],
  ['Business', '/business'],
  ['Reactor', '/docs/reactor'],
  ['Receipts', '/stats'],
  ['Docs', '/docs'],
] as const

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
] as const

const launchUiTheme = `
  :root {
    color-scheme: dark;
    --oa-launch-background: #050506;
    --oa-launch-foreground: #fafafa;
    --oa-launch-muted: rgb(214 211 209 / 0.72);
    --oa-launch-border: rgb(255 255 255 / 0.1);
    --oa-launch-brand: #2563eb;
    --oa-launch-brand-foreground: #38bdf8;
  }
  html {
    min-height: 100%;
    height: auto;
    overflow-x: hidden;
    overflow-y: auto !important;
    background: var(--oa-launch-background);
  }
  body {
    min-height: 100%;
    height: auto;
    overflow-x: hidden;
    overflow-y: auto !important;
    background: var(--oa-launch-background);
    font-family: InterVariable, Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }
`

const svg = (
  attrs: Record<string, unknown>,
  ...children: ReactNode[]
): ReactNode => h('svg', attrs, ...children)

const launchUiLogo = (className = 'size-6'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', {
      d: 'M10.5 12.75H3L6 9.75H9L15.75 3H20.25L10.5 12.75Z',
      fill: 'currentColor',
    }),
    h('path', {
      d: 'M11.25 15V13.5L21 3.75V5.25L11.25 15Z',
      fill: 'currentColor',
    }),
    h('path', {
      d: 'M11.25 18V16.5L21 6.75V8.25L11.25 18Z',
      fill: 'currentColor',
    }),
    h('path', {
      d: 'M11.25 21V19.5L15 15.75V17.25L11.25 21Z',
      fill: 'currentColor',
    }),
  )

const githubIcon = (className = 'size-4'): ReactNode =>
  svg(
    { className, height: 24, viewBox: '0 0 438.549 438.549', width: 24 },
    h('path', {
      d: 'M409.132 114.573c-19.608-33.596-46.205-60.194-79.798-79.8-33.598-19.607-70.277-29.408-110.063-29.408-39.781 0-76.472 9.804-110.063 29.408-33.596 19.605-60.192 46.204-79.8 79.8C9.803 148.168 0 184.854 0 224.63c0 47.78 13.94 90.745 41.827 128.906 27.884 38.164 63.906 64.572 108.063 79.227 5.14.954 8.945.283 11.419-1.996 2.475-2.282 3.711-5.14 3.711-8.562 0-.571-.049-5.708-.144-15.417a2549.81 2549.81 0 01-.144-25.406l-6.567 1.136c-4.187.767-9.469 1.092-15.846 1-6.374-.089-12.991-.757-19.842-1.999-6.854-1.231-13.229-4.086-19.13-8.559-5.898-4.473-10.085-10.328-12.56-17.556l-2.855-6.57c-1.903-4.374-4.899-9.233-8.992-14.559-4.093-5.331-8.232-8.945-12.419-10.848l-1.999-1.431c-1.332-.951-2.568-2.098-3.711-3.429-1.142-1.331-1.997-2.663-2.568-3.997-.572-1.335-.098-2.43 1.427-3.289 1.525-.859 4.281-1.276 8.28-1.276l5.708.853c3.807.763 8.516 3.042 14.133 6.851 5.614 3.806 10.229 8.754 13.846 14.842 4.38 7.806 9.657 13.754 15.846 17.847 6.184 4.093 12.419 6.136 18.699 6.136 6.28 0 11.704-.476 16.274-1.423 4.565-.952 8.848-2.383 12.847-4.285 1.713-12.758 6.377-22.559 13.988-29.41-10.848-1.14-20.601-2.857-29.264-5.14-8.658-2.286-17.605-5.996-26.835-11.14-9.235-5.137-16.896-11.516-22.985-19.126-6.09-7.614-11.088-17.61-14.987-29.979-3.901-12.374-5.852-26.648-5.852-42.826 0-23.035 7.52-42.637 22.557-58.817-7.044-17.318-6.379-36.732 1.997-58.24 5.52-1.715 13.706-.428 24.554 3.853 10.85 4.283 18.794 7.952 23.84 10.994 5.046 3.041 9.089 5.618 12.135 7.708 17.705-4.947 35.976-7.421 54.818-7.421s37.117 2.474 54.823 7.421l10.849-6.849c7.419-4.57 16.18-8.758 26.262-12.565 10.088-3.805 17.802-4.853 23.134-3.138 8.562 21.509 9.325 40.922 2.279 58.24 15.036 16.18 22.559 35.787 22.559 58.817 0 16.178-1.958 30.497-5.853 42.966-3.9 12.471-8.941 22.457-15.125 29.979-6.191 7.521-13.901 13.85-23.131 18.986-9.232 5.14-18.182 8.85-26.84 11.136-8.662 2.286-18.415 4.004-29.263 5.146 9.894 8.562 14.842 22.077 14.842 40.539v60.237c0 3.422 1.19 6.279 3.572 8.562 2.379 2.279 6.136 2.95 11.276 1.995 44.163-14.653 80.185-41.062 108.068-79.226 27.88-38.161 41.825-81.126 41.825-128.906-.01-39.771-9.818-76.454-29.414-110.049z',
      fill: 'currentColor',
    }),
  )

const xIcon = (className = 'h-3.5 w-3.5'): ReactNode =>
  svg(
    {
      className,
      height: 24,
      viewBox: '0 0 1200 1227',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', {
      d: 'M714.163 519.284L1160.89 0H1055.03L667.137 450.887L357.328 0H0L468.492 681.821L0 1226.37H105.866L515.491 750.218L842.672 1226.37H1200L714.137 519.284H714.163ZM569.165 687.828L521.697 619.934L144.011 79.6944H306.615L611.412 515.685L658.88 583.579L1055.08 1150.3H892.476L569.165 687.854V687.828Z',
      fill: 'currentColor',
    }),
  )

const moonIcon = (className = 'size-4'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 2,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', {
      d: 'M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z',
    }),
  )

const arrowRightIcon = (className = 'size-3'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 2,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', { d: 'M5 12h14' }),
    h('path', { d: 'm12 5 7 7-7 7' }),
  )

const sunIcon = (className = 'size-4'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 2,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('circle', { cx: '12', cy: '12', r: '4' }),
    h('path', { d: 'M12 2v2' }),
    h('path', { d: 'M12 20v2' }),
    h('path', { d: 'm4.93 4.93 1.41 1.41' }),
    h('path', { d: 'm17.66 17.66 1.41 1.41' }),
    h('path', { d: 'M2 12h2' }),
    h('path', { d: 'M20 12h2' }),
    h('path', { d: 'm6.34 17.66-1.41 1.41' }),
    h('path', { d: 'm19.07 4.93-1.41 1.41' }),
  )

const gridIcon = (className = 'size-4'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 1.5,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', { d: 'M4 4v16' }),
    h('path', { d: 'M9 4v16' }),
    h('path', { d: 'M15 4v16' }),
    h('path', { d: 'M20 4v16' }),
    h('path', { d: 'M4 4h16' }),
    h('path', { d: 'M4 9h16' }),
    h('path', { d: 'M4 15h16' }),
    h('path', { d: 'M4 20h16' }),
  )

const squareIcon = (className = 'size-4'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 1.5,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('rect', { height: '14', rx: '2', width: '14', x: '5', y: '5' }),
  )

const shuffleIcon = (className = 'size-4'): ReactNode =>
  svg(
    {
      className,
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 2,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('path', { d: 'm18 14 4 4-4 4' }),
    h('path', { d: 'm18 2 4 4-4 4' }),
    h('path', { d: 'M2 18h1.4c1.3 0 2.5-.6 3.3-1.7l6.6-8.6C14.1 6.6 15.3 6 16.6 6H22' }),
    h('path', { d: 'M2 6h1.4c1.3 0 2.5.6 3.3 1.7l1.2 1.6' }),
    h('path', { d: 'M22 18h-5.4c-1.3 0-2.5-.6-3.3-1.7l-1.2-1.6' }),
  )

const dotIcon = (): ReactNode =>
  svg(
    {
      className: 'hidden size-3 opacity-50 sm:block',
      fill: 'none',
      height: 24,
      stroke: 'currentColor',
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      strokeWidth: 2,
      viewBox: '0 0 24 24',
      width: 24,
      xmlns: 'http://www.w3.org/2000/svg',
    },
    h('circle', { cx: '12.1', cy: '12.1', r: '1' }),
  )

const toolbarButton = (
  label: string,
  children: ReactNode,
  active = false,
): ReactNode =>
  h(
    'button',
    {
      'aria-label': label,
      className: [
        'inline-flex h-10 w-10 items-center justify-center rounded-md p-1 text-sm font-medium transition-colors',
        active
          ? 'border border-white/20 bg-white/[0.08] text-white shadow-md'
          : 'bg-transparent text-stone-400 hover:bg-white/[0.06] hover:text-stone-300',
      ].join(' '),
      type: 'button',
    },
    children,
  )

const toolbarSwatch = (
  label: string,
  gradient: string,
  active = false,
): ReactNode =>
  toolbarButton(
    label,
    h('span', {
      className: `size-6 rounded-full ${gradient}`,
    }),
    active,
  )

const radiusSample = (
  label: string,
  radiusClass: string,
  active = false,
): ReactNode =>
  toolbarButton(
    label,
    h('span', {
      className: `size-5 border-2 border-r-0 border-b-0 border-stone-400/80 bg-white/10 ${radiusClass}`,
    }),
    active,
  )

const customizerToolbar = (): ReactNode =>
  h(
    'nav',
    {
      'aria-label': 'Preview customizer',
      className:
        'fixed bottom-3 left-1/2 z-50 hidden -translate-x-1/2 gap-2 rounded-[calc(0.25rem+0.5rem)] border border-white/10 border-t-white/25 bg-[#1b1b1d]/80 p-2 shadow-lg backdrop-blur-lg md:flex',
    },
    h(
      'div',
      { className: 'flex items-center gap-1 rounded-xl bg-white/[0.05] p-1' },
      toolbarSwatch('ember', 'bg-linear-to-b from-orange-400 to-orange-600'),
      toolbarSwatch('fire', 'bg-linear-to-b from-red-400 to-orange-500'),
      toolbarSwatch('ultraviolet', 'bg-linear-to-b from-indigo-400 to-violet-600'),
      toolbarSwatch('titanium', 'bg-linear-to-b from-stone-200 to-stone-500'),
      toolbarSwatch('ice', 'bg-linear-to-b from-sky-200 to-sky-500'),
      toolbarSwatch('holo', 'bg-linear-to-b from-sky-300 via-violet-400 to-amber-300'),
      toolbarSwatch('emerald', 'bg-linear-to-b from-emerald-300 to-teal-500'),
      toolbarSwatch('electro', 'bg-linear-to-b from-sky-400 to-blue-600', true),
    ),
    h(
      'div',
      { className: 'flex items-center gap-1 rounded-xl bg-white/[0.05] p-1' },
      radiusSample('small radius', 'rounded-[2px]'),
      radiusSample('default radius', 'rounded-[4px]', true),
      radiusSample('large radius', 'rounded-[10px]'),
      radiusSample('xl radius', 'rounded-[16px]'),
    ),
    h(
      'div',
      { className: 'flex items-center gap-1 rounded-xl bg-white/[0.05] p-1' },
      toolbarButton('Light', sunIcon()),
      toolbarButton('Dark', moonIcon(), true),
    ),
    h(
      'div',
      { className: 'flex items-center gap-1 rounded-xl bg-white/[0.05] p-1' },
      toolbarButton('Grid', gridIcon()),
      toolbarButton('Frame', squareIcon()),
    ),
    h(
      'button',
      {
        className:
          'inline-flex h-10 items-center justify-center gap-2 rounded-md bg-blue-500 px-5 text-sm font-medium text-[#00142d] shadow-md transition-colors hover:bg-blue-400',
        type: 'button',
      },
      shuffleIcon(),
      'Shuffle',
    ),
  )

const brandMark = (variant: LandingVariant): ReactNode =>
  variant === 'demo'
    ? launchUiLogo()
    : h(
        'span',
        {
          className:
            'inline-flex size-6 items-center justify-center rounded-[4px] border border-white/15 text-[0.68rem] font-semibold tracking-tight text-white',
        },
        'OA',
      )

const navIconLink = (
  label: string,
  href: string,
  icon: ReactNode,
): ReactNode =>
  h(
    'a',
    {
      'aria-label': label,
      className:
        'inline-flex size-9 items-center justify-center gap-2 rounded-md text-sm font-medium text-stone-300/85 transition-colors hover:bg-white/[0.06] hover:text-white',
      href,
    },
    h('span', { className: 'sr-only' }, label),
    icon,
  )

const promoBar = (variant: LandingVariant): ReactNode =>
  h(
    'div',
    {
      className:
        'relative top-0 z-20 w-full bg-[#0a0a0b] sm:fixed',
    },
    h(
      'a',
      {
        className:
          'group flex w-full items-center justify-center bg-[#050506] py-2.5 text-center text-sm font-normal text-stone-400 hover:bg-white/[0.03]',
        href: variant === 'demo' ? 'https://designwithcode.dev' : '/docs/product-promises',
        target: variant === 'demo' ? '_blank' : undefined,
        rel: variant === 'demo' ? 'noreferrer' : undefined,
      },
      h(
        'p',
        {
          className:
            'm-0 flex flex-col items-center gap-1 text-sm font-normal sm:flex-row',
        },
        h(
          'span',
          null,
          variant === 'demo'
            ? 'Start building top quality designs yourself'
            : 'Four products, one receipt spine',
        ),
        dotIcon(),
        h(
          'span',
          { className: 'flex flex-col items-center gap-1 sm:flex-row' },
          h(
            'span',
            null,
            variant === 'demo'
              ? h(React.Fragment, null, 'Check out my ', h('span', { className: 'text-white' }, 'free course'))
              : h(React.Fragment, null, 'Read the ', h('span', { className: 'text-white' }, 'public promise gates')),
          ),
          h(
            'strong',
            { className: 'flex items-center gap-1 font-medium text-[#38bdf8]' },
            h(
              'span',
              null,
              variant === 'demo' ? 'designwithcode.dev' : 'openagents.com',
            ),
            arrowRightIcon('size-3 transition-transform duration-300 group-hover:translate-x-1'),
          ),
        ),
      ),
    ),
  )

const navbar = (variant: LandingVariant): ReactNode => {
  const name = variant === 'demo' ? 'Launch UI' : 'OpenAgents'
  const version = variant === 'demo' ? 'v2.10' : 'suite'
  const links = variant === 'demo' ? launchUiNavItems : openAgentsNavItems

  return h(
    'nav',
    {
      className:
        'sticky top-0 z-50 h-16 w-full border-t border-white/10 bg-[#050506]/5 backdrop-blur-lg sm:top-10',
    },
    h(
      'div',
      {
        className:
          'mx-auto flex h-full max-w-[1536px] items-center justify-between px-4 md:gap-2',
      },
      h(
        'div',
        { className: 'flex items-center gap-3' },
        h(
          'div',
          { className: 'flex items-center gap-6' },
          h(
            'div',
            { className: 'flex items-center gap-2' },
            h(
              'a',
              {
                className:
                  'flex items-center gap-2.5 text-white no-underline',
                href: variant === 'demo' ? '/demo' : '/new',
              },
              brandMark(variant),
              h('h2', { className: 'm-0 text-[1rem] font-bold' }, name),
            ),
            h(
              'a',
              { href: variant === 'demo' ? '/docs/getting-started/introduction' : '/docs' },
              h(
                'span',
                {
                  className:
                    'inline-flex items-center rounded-full border border-white/20 px-2.5 py-1 text-xs font-semibold text-stone-300/85 transition-colors hover:bg-white/[0.06]',
                },
                version,
              ),
            ),
          ),
          h(
            'div',
            {
              className:
                'hidden items-center gap-6 text-sm font-medium text-stone-300/85 md:flex',
            },
            ...links.map(([text, href]) =>
              h(
                'a',
                {
                  className:
                    'flex items-center gap-1 text-stone-300/85 no-underline transition-colors hover:text-white',
                  href,
                  key: text,
                },
                text,
              ),
            ),
          ),
        ),
      ),
      h(
        'div',
        { className: 'ml-2.5 flex sm:ml-0' },
        navIconLink(
          'GitHub',
          variant === 'demo' ? 'https://github.com/launch-ui/launch-ui' : '/docs',
          githubIcon(),
        ),
        navIconLink(
          'Twitter',
          variant === 'demo' ? 'https://twitter.com/mikolajdobrucki' : '/forum',
          xIcon(),
        ),
        navIconLink('Dark mode', '#', moonIcon()),
      ),
    ),
  )
}

const badge = (variant: LandingVariant): ReactNode =>
  h(
    'span',
    {
      className:
        'inline-flex items-center gap-2 rounded-full border border-white/20 px-3 py-1.5 text-xs font-semibold text-white',
      'data-slot': 'badge',
    },
    h(
      'span',
      { className: 'text-stone-400' },
      variant === 'demo' ? 'Launch UI v2 is out!' : 'Four products, one receipt spine.',
    ),
    h(
      'a',
      {
        className: 'flex items-center gap-1 text-white no-underline',
        href: variant === 'demo' ? '/docs/getting-started/introduction' : '/docs/product-promises',
      },
      variant === 'demo' ? 'Read more' : 'Read promises',
      h('span', { 'aria-hidden': 'true' }, '→'),
    ),
  )

const primaryButton = (text: string, href: string): ReactNode =>
  h(
    'a',
    {
      className:
        'inline-flex h-10 items-center justify-center gap-2 whitespace-nowrap rounded-md border-t border-white/70 bg-linear-to-b from-white/60 to-white px-5 text-sm font-medium text-[#111827] no-underline shadow-sm transition-colors hover:from-white/80 hover:to-white/90',
      href,
    },
    text,
  )

const glowButton = (text: string, href: string): ReactNode =>
  h(
    'a',
    {
      className:
        'inline-flex h-10 items-center justify-center gap-2 whitespace-nowrap rounded-md border border-white/10 border-t-white/30 bg-linear-to-b from-white/10 to-white/[0.03] px-5 text-sm font-medium text-white no-underline shadow-md transition-colors hover:from-white/15 hover:to-white/[0.06]',
      href,
    },
    githubIcon('mr-1 size-4'),
    text,
  )

const avatarImages = [
  'https://i.pravatar.cc/64?img=12',
  'https://i.pravatar.cc/64?img=47',
  'https://i.pravatar.cc/64?img=32',
] as const

const socialProof = (variant: LandingVariant): ReactNode =>
  h(
    'div',
    {
      className:
        'flex flex-wrap items-center gap-3 text-sm font-medium text-stone-400',
    },
    h(
      'div',
      { className: 'flex -space-x-3' },
      ...avatarImages.map((src, index) =>
        h('img', {
          alt: '',
          className:
            'size-8 rounded-full border-2 border-[#050506] bg-stone-800 object-cover shadow-md',
          height: 32,
          key: src,
          src,
          width: 32,
          'data-avatar-index': index,
        }),
      ),
    ),
    h(
      'div',
      { className: 'flex items-center gap-0.5 text-white' },
      ...Array.from({ length: 5 }, (_, index) =>
        h('span', { 'aria-hidden': 'true', className: 'text-sm', key: index }, '★'),
      ),
    ),
    h(
      'p',
      { className: 'm-0 text-sm font-medium text-stone-400' },
      variant === 'demo'
        ? 'Used by 34.7k+ companies and builders'
        : 'Counters are projections of receipts, not estimates.',
    ),
  )

const demoMockup = (): ReactNode =>
  h(
    'div',
    {
      className:
        'pointer-events-none absolute inset-x-0 bottom-[-260px] z-0 h-[650px] sm:bottom-[-310px] lg:bottom-[-350px]',
    },
    h('div', {
      className:
        'absolute left-1/2 top-14 h-[360px] w-[70%] -translate-x-1/2 scale-[2.5] rounded-[50%] bg-radial from-[#38bdf8]/50 from-10% to-[#38bdf8]/0 to-60% opacity-90 blur-sm sm:h-[512px]',
    }),
    h('div', {
      className:
        'absolute left-1/2 top-22 h-[220px] w-[46%] -translate-x-1/2 scale-200 rounded-[50%] bg-radial from-[#2563eb]/35 from-10% to-[#38bdf8]/0 to-60% opacity-100',
    }),
    h(
      'div',
      {
        className:
          'absolute left-1/2 top-16 w-[1500px] max-w-[152vw] -translate-x-[42%] rotate-[-11deg] sm:top-8 lg:top-0',
      },
      h(
        'div',
        {
          className:
            'relative z-10 flex overflow-hidden rounded-[18px] bg-[#2563eb]/15 p-2 shadow-[-12px_16px_48px_#00000088]',
        },
        h(
          'div',
          {
            className:
              'relative z-10 flex w-full overflow-hidden rounded-[12px] border border-white/5 border-t-white/15 bg-[#050506]/90 shadow-2xl',
          },
          h('img', {
            alt: 'Launch UI app screenshot',
            className: 'block h-auto w-full',
            height: 765,
            loading: 'eager',
            src: '/dashboard-dark.png',
            width: 1248,
          }),
        ),
      ),
    ),
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
    {
      className:
        'pointer-events-none absolute inset-x-0 bottom-[-250px] z-0 h-[650px] sm:bottom-[-300px] lg:bottom-[-340px]',
    },
    h('div', {
      className:
        'absolute left-1/2 top-14 h-[360px] w-[70%] -translate-x-1/2 scale-[2.5] rounded-[50%] bg-radial from-[#38bdf8]/45 from-10% to-[#38bdf8]/0 to-60% opacity-90 blur-sm sm:h-[512px]',
    }),
    h('div', {
      className:
        'absolute left-1/2 top-22 h-[220px] w-[46%] -translate-x-1/2 scale-200 rounded-[50%] bg-radial from-[#2563eb]/35 from-10% to-[#38bdf8]/0 to-60% opacity-100',
    }),
    h(
      'div',
      {
        className:
          'absolute left-1/2 top-16 w-[1500px] max-w-[152vw] -translate-x-[42%] rotate-[-11deg] sm:top-8 lg:top-0',
      },
      h(
        'div',
        {
          className:
            'relative z-10 overflow-hidden rounded-[18px] bg-[#2563eb]/15 p-2 shadow-[-12px_16px_48px_#00000088]',
        },
        h(
          'div',
          {
            className:
              'relative z-10 overflow-hidden rounded-[12px] border border-white/5 border-t-white/15 bg-[#050506]/90 p-5 shadow-2xl',
          },
          h(
            'div',
            { className: 'grid gap-3 sm:grid-cols-2' },
            ...productSurfaces.map(productSurfaceRow),
          ),
          h('img', {
            alt: '',
            className: 'mt-4 block h-auto w-full rounded-[8px] opacity-45',
            height: 765,
            loading: 'eager',
            src: '/dashboard-dark.png',
            width: 1248,
          }),
        ),
      ),
    ),
  )

const hero = (variant: LandingVariant): ReactNode => {
  const isDemo = variant === 'demo'
  const title = isDemo
    ? 'Give your big idea the design it deserves'
    : 'The operating system for agents that work'
  const description = isDemo
    ? 'Professionally designed blocks and templates built with React, Shadcn/ui and Tailwind that will help your product stand out.'
    : 'Khala Code on mobile, Khala Code on desktop, openagents.com for business control, and Reactor for private model sovereignty. One suite, one metered substrate, one public-safe receipt trail.'

  return h(
    'section',
    {
      className:
        'relative min-h-[1320px] px-4 pt-44 pb-0 sm:min-h-[1280px] sm:pt-52 md:min-h-[1220px] md:pt-56',
    },
    h(
      'div',
      { className: 'mx-auto max-w-[1280px]' },
      h(
        'div',
        {
          className:
            'relative z-10 flex max-w-[760px] flex-col items-start gap-7 text-left sm:gap-9',
        },
        badge(variant),
        h(
          'h1',
          {
            className:
              'relative z-10 m-0 inline-block max-w-[960px] bg-linear-to-r from-white to-stone-400 bg-clip-text text-5xl font-semibold tracking-tight text-balance text-transparent drop-shadow-2xl sm:text-6xl md:text-7xl',
          },
          title,
        ),
        h(
          'p',
          {
            className:
              'relative z-10 m-0 max-w-[660px] text-lg font-medium text-pretty text-stone-400 sm:text-xl',
          },
          description,
        ),
        h(
          'div',
          { className: 'relative z-10 flex flex-wrap justify-start gap-4' },
          primaryButton(isDemo ? 'Get Started' : 'Open business', isDemo ? '/docs/getting-started/introduction' : '/business'),
          glowButton(isDemo ? 'Github' : 'Docs', isDemo ? 'https://github.com/launch-ui/launch-ui' : '/docs'),
        ),
        socialProof(variant),
      ),
      isDemo ? demoMockup() : newMockup(),
    ),
  )
}

const sectionShell = (
  children: ReactNode,
  className = '',
  key?: string,
): ReactNode =>
  h(
    'section',
    {
      className: `relative px-4 py-20 sm:py-28 ${className}`,
      key,
    },
    h('div', { className: 'mx-auto max-w-[1280px]' }, children),
  )

const logoCloud = (variant: LandingVariant): ReactNode => {
  const names =
    variant === 'demo'
      ? ['Figma', 'React', 'TypeScript', 'Shadcn/ui', 'Tailwind']
      : ['Khala Code mobile', 'Khala Code desktop', 'openagents.com', 'Reactor']

  return sectionShell(
    h(
      'div',
      { className: 'flex flex-col items-center gap-8 text-center' },
      h(
        'span',
        {
          className:
            'inline-flex items-center rounded-full border border-[#38bdf8]/30 px-2.5 py-1 text-xs font-semibold text-[#38bdf8]',
        },
        variant === 'demo' ? 'Last updated: 2026' : 'Receipt-backed surfaces',
      ),
      h(
        'h2',
        { className: 'm-0 text-[1rem] font-semibold text-white sm:text-2xl' },
        variant === 'demo'
          ? 'Built with industry-standard tools and best practices'
          : 'Four products, one operational substrate',
      ),
      h(
        'div',
        { className: 'flex flex-wrap items-center justify-center gap-8' },
        ...names.map(name =>
          h(
            'div',
            {
              className:
                'flex items-center gap-2 rounded-md border border-white/10 bg-white/[0.03] px-4 py-3 text-sm font-medium text-stone-300',
              key: name,
            },
            h('span', {
              className:
                'size-2 rounded-full bg-[#38bdf8] shadow-[0_0_18px_rgba(56,189,248,0.75)]',
            }),
            name,
          ),
        ),
      ),
    ),
  )
}

const featureItems = (variant: LandingVariant): ReactNode => {
  const items =
    variant === 'demo'
      ? [
          ['Copy/paste blocks', 'A complete landing kit assembled from React and Tailwind primitives.'],
          ['Production ready', 'Sections are composed for real launch pages, not toy demos.'],
          ['Customizable tokens', 'Color, radius, mode, and layout presets stay visible in the preview.'],
        ]
      : productSurfaces.map(product => [product.name, product.detail])

  return sectionShell(
    h(
      'div',
      { className: 'grid gap-8 md:grid-cols-[0.8fr_1.2fr]' },
      h(
        'div',
        { className: 'max-w-[560px]' },
        h(
          'h2',
          { className: 'm-0 text-3xl font-semibold tracking-tight text-white sm:text-5xl' },
          variant === 'demo'
            ? 'Everything below the fold is part of the system.'
            : 'The same composition, adapted to OpenAgents.',
        ),
        h(
          'p',
          { className: 'mt-5 text-lg font-medium text-stone-400' },
          variant === 'demo'
            ? 'The original lander continues into reusable sections, metrics, pricing, FAQs, and a footer.'
            : 'The preview preserves the Launch UI layout language while replacing the story with the four-prong product suite.',
        ),
      ),
      h(
        'div',
        { className: 'grid gap-4 sm:grid-cols-2' },
        ...items.map(([title, copy]) =>
          h(
            'article',
            {
              className:
                'rounded-xl border border-white/10 border-t-white/20 bg-linear-to-b from-white/[0.07] to-white/[0.02] p-5 shadow-xl',
              key: title,
            },
            h('h3', { className: 'm-0 text-lg font-semibold text-white' }, title),
            h('p', { className: 'm-0 mt-3 text-base leading-7 text-stone-400 sm:text-sm sm:leading-6' }, copy),
          ),
        ),
      ),
    ),
  )
}

const statsSection = (variant: LandingVariant): ReactNode => {
  const stats =
    variant === 'demo'
      ? [
          ['34.7k+', 'companies and builders'],
          ['100+', 'components and blocks'],
          ['React 19', 'modern app foundation'],
        ]
      : [
          ['Exact', 'usage truth'],
          ['Typed', 'authority boundary'],
          ['Public-safe', 'receipt trail'],
        ]

  return sectionShell(
    h(
      'div',
      { className: 'grid gap-4 sm:grid-cols-3' },
      ...stats.map(([value, label]) =>
        h(
          'div',
          {
            className:
              'rounded-xl border border-white/10 border-t-white/20 bg-linear-to-b from-white/[0.07] to-white/[0.02] p-6 shadow-xl',
            key: value,
          },
          h('p', { className: 'm-0 text-4xl font-semibold tracking-tight text-white' }, value),
          h('p', { className: 'm-0 mt-2 text-sm font-medium text-stone-400' }, label),
        ),
      ),
    ),
  )
}

const pricingSection = (variant: LandingVariant): ReactNode =>
  sectionShell(
    h(
      'div',
      { className: 'rounded-2xl border border-white/10 border-t-white/20 bg-linear-to-b from-white/[0.08] to-white/[0.025] p-8 shadow-2xl' },
      h(
        'div',
        { className: 'grid gap-8 md:grid-cols-[1fr_auto]' },
        h(
          'div',
          null,
          h(
            'h2',
            { className: 'm-0 text-3xl font-semibold tracking-tight text-white sm:text-5xl' },
            variant === 'demo' ? 'Start with the landing kit.' : 'Start with the business surface.',
          ),
          h(
            'p',
            { className: 'm-0 mt-4 max-w-[720px] text-lg font-medium text-stone-400' },
            variant === 'demo'
              ? 'Pricing, FAQ, CTA, and footer sections continue the same Launch UI rhythm below the hero.'
              : 'Pricing and live counters stay owner-gated; this route keeps the layout ready without overstating what is live.',
          ),
        ),
        h(
          'div',
          { className: 'flex items-center md:justify-end' },
          primaryButton(variant === 'demo' ? 'Get Started' : 'Open business', variant === 'demo' ? '/docs/getting-started/introduction' : '/business'),
        ),
      ),
    ),
  )

const footerSection = (variant: LandingVariant): ReactNode =>
  h(
    'footer',
    { className: 'border-t border-white/10 px-4 py-12' },
    h(
      'div',
      { className: 'mx-auto flex max-w-[1280px] flex-col gap-6 text-sm text-stone-400 md:flex-row md:items-center md:justify-between' },
      h(
        'div',
        { className: 'flex items-center gap-2 text-white' },
        brandMark(variant),
        h('span', { className: 'font-semibold' }, variant === 'demo' ? 'Launch UI' : 'OpenAgents'),
      ),
      h(
        'div',
        { className: 'flex flex-wrap gap-5' },
        ...(variant === 'demo' ? launchUiNavItems : openAgentsNavItems).map(([text, href]) =>
          h('a', { className: 'text-stone-400 no-underline hover:text-white', href, key: text }, text),
        ),
      ),
    ),
  )

const pageSections = (variant: LandingVariant): ReactNode[] => [
  logoCloud(variant),
  featureItems(variant),
  statsSection(variant),
  pricingSection(variant),
  footerSection(variant),
]

const shell = (
  variant: LandingVariant,
  title: string,
  description: string,
  body: ReactNode,
): string => `<!doctype html>${renderToStaticMarkup(
  h(
    'html',
    {
      className: 'dark',
      lang: 'en',
      style: { colorScheme: 'dark' },
    },
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
      h('link', { href: 'https://rsms.me/inter/inter.css', rel: 'stylesheet' }),
      h('link', { href: '/favicon.svg', rel: 'icon', type: 'image/svg+xml' }),
      h('link', { href: '/assets/openagents.css', rel: 'stylesheet' }),
      h('style', {
        dangerouslySetInnerHTML: { __html: launchUiTheme },
      }),
    ),
    h(
      'body',
      {
        className: 'm-0 min-h-screen bg-[#050506] text-white antialiased',
        'data-react-landing-route': variant,
      },
      body,
    ),
  ),
)}`

export const renderReactLandingHtml = (variant: LandingVariant): string =>
  shell(
    variant,
    variant === 'demo' ? 'Launch UI Replica' : 'OpenAgents Suite',
    variant === 'demo'
      ? 'Exact original Launch UI replica preserved for comparison.'
      : 'OpenAgents suite preview covering Khala Code mobile, Khala Code desktop, openagents.com, and Reactor.',
    h(
      React.Fragment,
      null,
      promoBar(variant),
      h(
        'div',
        {
          className: 'flex min-h-screen w-full flex-col bg-[#050506] text-white',
        },
        navbar(variant),
        h(
          'main',
          {
            className: 'w-full bg-[#050506] text-white',
            'data-route': variant === 'demo' ? 'demo-landing' : 'new-landing',
          },
          hero(variant),
          ...pageSections(variant),
        ),
        customizerToolbar(),
      ),
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
