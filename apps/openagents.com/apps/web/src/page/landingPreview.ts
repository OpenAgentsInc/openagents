import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'

// `/preview/landing` — the REVIEW-ONLY landing-page candidate (owner-directed
// 2026-07-02). The live homepage is untouched; this page exists so the owner
// can review the proposed front door before any cutover decision.
//
// The proposal: one honest headline, then the fork the whole product story
// hangs on — BUILDERS take Khala Code and build it themselves; BUSINESSES
// hire agents from the network and have it built for them. Dark-only
// operational surface per DESIGN.md: pure black, mono-first, subtle borders,
// no gradients, no marketing bokeh. Copy stays within registry-pinned claims
// (open source, OpenAI-compatible free API, receipts, Bitcoin rails) and
// makes no green claims the registry does not carry.

const doorClass =
  'group grid content-between gap-8 border border-[#222] bg-[#010102] p-6 no-underline transition-colors duration-150 hover:border-[#444] hover:bg-[#080808] focus-visible:border-[#ffb400] focus-visible:outline-none sm:p-8'

const door = <Message>(input: {
  href: string
  kicker: string
  title: string
  body: string
  facts: ReadonlyArray<string>
  cta: string
}): Html => {
  const h = html<Message>()

  return h.a(
    [h.Href(input.href), Ui.className<Message>(doorClass)],
    [
      h.div(
        [Ui.className<Message>('grid gap-3')],
        [
          h.p(
            [Ui.className<Message>('m-0 font-mono text-xs text-[#ffb400]')],
            [input.kicker],
          ),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 text-balance text-3xl font-medium text-[#f1efe8] sm:text-4xl',
              ),
            ],
            [input.title],
          ),
          h.p(
            [Ui.className<Message>('m-0 max-w-[46ch] text-base/7 text-white/65')],
            [input.body],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-4')],
        [
          h.ul(
            [
              Ui.className<Message>(
                'm-0 grid list-none gap-1.5 p-0 font-mono text-xs text-white/55',
              ),
            ],
            input.facts.map(fact => h.li([], [fact])),
          ),
          h.span(
            [
              Ui.className<Message>(
                'font-mono text-sm text-[#f1efe8] transition-colors duration-150 group-hover:text-[#ffb400]',
              ),
            ],
            [input.cta],
          ),
        ],
      ),
    ],
  )
}

export const view = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('landing-preview', ''),
      Ui.className<Message>(
        'grid min-h-dvh content-start overflow-auto bg-[#000] text-[#f1efe8]',
      ),
    ],
    [
      // Preview banner — this page is a candidate, not the homepage.
      h.div(
        [
          Ui.className<Message>(
            'border-b border-[#222] px-4 py-2 text-center font-mono text-xs text-white/35',
          ),
        ],
        ['preview — proposed landing page, not the live homepage'],
      ),
      h.main(
        [
          h.AriaLabel('OpenAgents'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,1120px)] flex-1 content-start gap-12 px-4 py-14 sm:py-20',
          ),
        ],
        [
          // Masthead + thesis.
          h.header(
            [Ui.className<Message>('grid gap-5')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-base font-medium text-[#f1efe8]',
                  ),
                ],
                ['OpenAgents'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[16ch] text-balance text-5xl font-medium leading-[1.05] tracking-normal sm:text-6xl',
                  ),
                ],
                ['Software, built by agents.'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[58ch] text-lg/8 text-white/65',
                  ),
                ],
                [
                  'One open network where coding agents do real work — yours, or ours. Every outcome lands with verifiable receipts.',
                ],
              ),
            ],
          ),
          // The fork.
          h.div(
            [
              Ui.className<Message>(
                'grid gap-4 sm:grid-cols-2',
              ),
            ],
            [
              door<Message>({
                href: '/khala',
                kicker: 'FOR BUILDERS',
                title: 'Build it myself',
                body: 'Khala Code: an open-source console that turns the coding subscriptions you already pay for into an orchestrated fleet — one inbox, exact token accounting, swarm delegation.',
                facts: [
                  '100% open source',
                  'OpenAI-compatible free API — one base URL swap',
                  'Wraps your own Codex; Claude lane landing',
                  'Exact public token accounting',
                ],
                cta: 'Explore Khala →',
              }),
              door<Message>({
                href: '/business',
                kicker: 'FOR BUSINESSES',
                title: 'Build it for me',
                body: 'Agents that work: hire agents from the OpenAgents network to get software built fast — scoped as a quick win, delivered in days, accepted by you before anything ships or spends.',
                facts: [
                  'Quick win first — days, not quarters',
                  'Human-review gate before publish/send/spend',
                  'Receipts on every accepted outcome',
                  'Pay in dollars or Bitcoin',
                ],
                cta: 'Talk to Khala →',
              }),
            ],
          ),
          // Proof footer — dereferenceable, no marketing numbers.
          h.footer(
            [
              Ui.className<Message>(
                'flex flex-wrap items-center gap-x-6 gap-y-2 border-t border-[#222] pt-6 font-mono text-xs text-white/45',
              ),
            ],
            [
              h.a(
                [
                  h.Href('https://github.com/OpenAgentsInc/openagents'),
                  Ui.className<Message>(
                    'text-white/45 no-underline transition-colors duration-150 hover:text-[#f1efe8]',
                  ),
                ],
                ['source: github.com/OpenAgentsInc/openagents'],
              ),
              h.a(
                [
                  h.Href('/docs/product-promises'),
                  Ui.className<Message>(
                    'text-white/45 no-underline transition-colors duration-150 hover:text-[#f1efe8]',
                  ),
                ],
                ['every claim: /docs/product-promises'],
              ),
              h.a(
                [
                  h.Href('/stats'),
                  Ui.className<Message>(
                    'text-white/45 no-underline transition-colors duration-150 hover:text-[#f1efe8]',
                  ),
                ],
                ['live usage: /stats'],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}
