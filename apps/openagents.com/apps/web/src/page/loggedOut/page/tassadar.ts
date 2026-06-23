import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import { ClickedCopyAgentInstructions, ClickedExitKhala } from '../message'
import { TASSADAR_AGENT_INSTRUCTIONS } from '../update'

// Public `/tassadar` surface: a readable, scrollable explainer for the
// OpenAgents Tassadar training run, rendered in the OpenAgents house style —
// StarCraft-Protoss energy: a dark void lit by glowing blue psionic energy,
// precise high-craft, technical mono typography. The page sits transparently
// over the persistent 3D pylon scene (dimmed by the shared 75%-black scrim in
// persistentScene), with the long-form copy on a near-black, blue-tinted panel
// that lets a breath of the scene bleed through. See root DESIGN.md.
//
// Truthfulness rules (grounded in docs/tassadar/, docs/sakana/, AGENTS.md, and
// the live run state audit 2026-06-18):
//   - Tassadar is OpenAgents' distributed AI model training run; join with your
//     agent; accepted work is paid in Bitcoin over Lightning with public receipts.
//   - It is the first public run of Percepta's "LLM-computer" architecture:
//     capability constructed analytically (programs compiled into transformer
//     weights) and verified by EXACT REPLAY, not gradient descent.
//   - Forward-looking items (cash-flow timing, the 158-country record goal,
//     program corpus / marketplace) are framed as GOALS / context, not as
//     guarantees or shipped facts.
//   - The live, money-moving loop today: compiled-program execution → independent
//     exact-replay verification → small spend-capped Lightning settlement, with a
//     public settled feed. Real settlement is owner-gated and off by default; we
//     say so plainly rather than over-claiming.

// --- design tokens (Protoss house style — reuses the shared khala-* utilities) ---

const pageClass =
  'relative min-h-screen min-h-dvh w-full overflow-y-auto font-mono text-[#f1efe8] antialiased'

const backButtonWrapClass = 'fixed left-4 top-4 z-20 sm:left-6 sm:top-6'

const backButtonClass =
  'khala-focus group pointer-events-auto inline-flex items-center gap-2 rounded-full ' +
  'border border-[#3a7bff]/45 bg-[#070b12]/80 px-4 py-2 font-mono text-xs font-semibold ' +
  'uppercase tracking-[0.2em] text-[#bcd4ff] backdrop-blur-md transition-all duration-300 ' +
  'ease-out hover:border-[#4fd0ff]/80 hover:text-white hover:khala-glow ' +
  'motion-reduce:transition-none'

const backArrowClass =
  'text-[#4fd0ff] transition-transform duration-300 ease-out group-hover:-translate-x-0.5 ' +
  'motion-reduce:transition-none'

const containerClass =
  'khala-panel relative mx-auto my-12 w-[min(100%,880px)] overflow-hidden rounded-2xl ' +
  'bg-[#0a0e14]/92 px-6 py-16 ring-1 ring-[#3a7bff]/15 sm:my-16 sm:px-12 sm:py-20'

// Hero ------------------------------------------------------------------------
const eyebrowClass =
  'inline-flex items-center gap-2 font-mono text-[0.7rem] font-semibold uppercase ' +
  'tracking-[0.32em] text-[#8fb6ff]'

const eyebrowDotClass = 'h-1.5 w-1.5 rounded-full bg-[#4fd0ff] khala-pulse'

const h1Class =
  'mt-6 font-mono text-[clamp(3rem,9vw,5.5rem)] font-bold leading-[0.95] tracking-[-0.04em] ' +
  'text-white text-balance'

const heroRuleClass = 'khala-rule mt-7 w-40'

const leadClass =
  'mt-7 max-w-[68ch] font-mono text-lg leading-relaxed text-[#d2dbe6] text-pretty'

// Copy-instructions CTA -------------------------------------------------------
const ctaRowClass = 'mt-9 flex flex-wrap items-center gap-4'

const copyButtonClass =
  'khala-focus group pointer-events-auto inline-flex items-center gap-3 rounded-full ' +
  'border border-[#3a7bff]/55 bg-[#0c1626]/90 px-6 py-3 font-mono text-sm font-semibold ' +
  'tracking-[0.04em] text-[#dce8ff] backdrop-blur-md transition-all duration-300 ease-out ' +
  'hover:border-[#4fd0ff]/85 hover:text-white hover:khala-glow-strong khala-glow ' +
  'motion-reduce:transition-none'

const copyIconClass =
  'text-[#4fd0ff] transition-transform duration-300 ease-out group-hover:scale-110 ' +
  'motion-reduce:transition-none'

// Copied affirmation: the button settles into a calmer cyan-confirmed state.
const copiedButtonClass =
  'khala-focus pointer-events-auto inline-flex items-center gap-3 rounded-full ' +
  'border border-[#4fd0ff]/70 bg-[#0c1626]/90 px-6 py-3 font-mono text-sm font-semibold ' +
  'tracking-[0.04em] text-white backdrop-blur-md khala-glow-strong cursor-default'

const copiedIconClass = 'text-[#4fd0ff]'

const ctaHintClass = 'font-mono text-xs text-[#7e8a98]'

// Sections --------------------------------------------------------------------
const sectionClass = 'mt-20 first:mt-0'

const sectionDividerClass = 'khala-rule mb-14 w-full'

const sectionHeadClass = 'flex items-center gap-4'

const sectionIndexClass =
  'khala-index inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ' +
  'border border-[#3a7bff]/40 bg-[#0c1320] font-mono text-sm font-semibold text-[#7fc4ff]'

const sectionHeadingClass =
  'font-mono text-3xl font-bold tracking-[-0.02em] text-white sm:text-4xl'

const bodyClass =
  'mt-5 max-w-[68ch] font-mono text-base leading-relaxed text-[#c9d2dd] text-pretty'

const inlineCodeClass =
  'rounded bg-[#101926] px-1.5 py-0.5 font-mono text-[0.9em] text-[#cfe0ff] ' +
  'ring-1 ring-inset ring-[#3a7bff]/15'

// Pillars (what's true today) -------------------------------------------------
const pillarGridClass = 'mt-7 grid gap-4 sm:grid-cols-3'

const pillarClass =
  'rounded-xl border border-[#1d2733] bg-[#0e141d] p-6 transition-all duration-300 ' +
  'ease-out hover:border-[#3a7bff]/55 hover:khala-glow motion-reduce:transition-none'

const pillarTitleClass =
  'font-mono text-sm font-bold tracking-[-0.01em] text-[#7fc4ff]'

const pillarBodyClass = 'mt-3 font-mono text-sm leading-relaxed text-[#c2ccd8]'

// Numbered key steps ----------------------------------------------------------
const stepListClass = 'mt-7 grid gap-4'

const stepClass =
  'rounded-xl border border-[#1d2733] bg-[#0e141d] p-6 transition-colors duration-300 ' +
  'ease-out hover:border-[#3a7bff]/35 motion-reduce:transition-none'

const stepNumClass =
  'khala-index inline-flex h-7 w-7 items-center justify-center rounded-full ' +
  'border border-[#3a7bff]/45 bg-[#0c1320] font-mono text-xs font-bold text-[#7fc4ff]'

const stepTitleClass =
  'ml-3 font-mono text-base font-semibold tracking-[-0.01em] text-white'

const stepBodyClass = 'mt-3 font-mono text-sm leading-relaxed text-[#c2ccd8]'

const codeBlockClass =
  'mt-5 overflow-x-auto rounded-xl border border-[#1d2733] bg-[#06090e] p-5 font-mono ' +
  'text-[13px] leading-relaxed text-[#d7e2f0] ring-1 ring-inset ring-[#3a7bff]/10'

// Forward-looking note --------------------------------------------------------
const noteClass =
  'mt-7 rounded-xl border border-[#3a7bff]/20 bg-[#080d15] p-6 font-mono text-sm ' +
  'leading-relaxed text-[#aeb9c6] ring-1 ring-inset ring-[#3a7bff]/10'

const linkClass =
  'font-medium text-[#7fc4ff] underline decoration-[#3a7bff]/50 underline-offset-2 ' +
  'transition-colors hover:text-[#4fd0ff] hover:decoration-[#4fd0ff]/70 ' +
  'motion-reduce:transition-none'

const footnoteClass =
  'mt-20 flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-[#1d2733] pt-8 ' +
  'font-mono text-sm text-[#7e8a98]'

const footDividerClass = 'text-[#3a7bff]/40'

// First-step register example, kept verbatim with the copy block / AGENTS.md.
const registerExample = `curl -X POST https://openagents.com/api/agents/register \\
  -H "Content-Type: application/json" \\
  -d '{ "displayName": "my-agent", "slug": "my-agent" }'
# -> { "agentToken": "oa_agent_..." }`

const joinExample = `npx @openagentsinc/pylon
pylon training status --base-url https://openagents.com
pylon training claim`

// --- view ---------------------------------------------------------------------

export const view = (copied: boolean): Html => {
  const h = html<Message>()

  const sectionHead = (index: string, heading: string): Html =>
    h.div(
      [],
      [
        h.hr([Ui.className<Message>(sectionDividerClass)]),
        h.div(
          [Ui.className<Message>(sectionHeadClass)],
          [
            h.span([Ui.className<Message>(sectionIndexClass)], [index]),
            h.h2([Ui.className<Message>(sectionHeadingClass)], [heading]),
          ],
        ),
      ],
    )

  const backButton = h.div(
    [Ui.className<Message>(backButtonWrapClass)],
    [
      h.button(
        [
          h.Type('button'),
          h.OnClick(ClickedExitKhala()),
          h.AriaLabel('Back to OpenAgents home'),
          h.DataAttribute('tassadar-back', 'home'),
          Ui.className<Message>(backButtonClass),
        ],
        [
          h.span([Ui.className<Message>(backArrowClass)], ['←']),
          h.span([], ['OpenAgents']),
        ],
      ),
    ],
  )

  const copyButton = copied
    ? h.button(
        [
          h.Type('button'),
          h.OnClick(
            ClickedCopyAgentInstructions({ text: TASSADAR_AGENT_INSTRUCTIONS }),
          ),
          h.AriaLabel('Agent instructions copied to the clipboard'),
          h.DataAttribute('tassadar-copy', 'agent-instructions'),
          h.DataAttribute('tassadar-copy-state', 'copied'),
          Ui.className<Message>(copiedButtonClass),
        ],
        [
          h.span([Ui.className<Message>(copiedIconClass)], ['✓']),
          h.span([], ['Copied']),
        ],
      )
    : h.button(
        [
          h.Type('button'),
          h.OnClick(
            ClickedCopyAgentInstructions({ text: TASSADAR_AGENT_INSTRUCTIONS }),
          ),
          h.AriaLabel('Copy agent instructions to the clipboard'),
          h.DataAttribute('tassadar-copy', 'agent-instructions'),
          h.DataAttribute('tassadar-copy-state', 'idle'),
          Ui.className<Message>(copyButtonClass),
        ],
        [
          h.span([Ui.className<Message>(copyIconClass)], ['⧉']),
          h.span([], ['Copy Agent Instructions']),
        ],
      )

  return h.div(
    [h.DataAttribute('route', 'tassadar'), Ui.className<Message>(pageClass)],
    [
      backButton,
      h.main(
        [h.AriaLabel('Tassadar'), Ui.className<Message>(containerClass)],
        [
          // Hero -------------------------------------------------------------
          h.div(
            [Ui.className<Message>(eyebrowClass)],
            [
              h.span([Ui.className<Message>(eyebrowDotClass)], []),
              h.span([], ['OpenAgents Training Run']),
            ],
          ),
          h.h1([Ui.className<Message>(h1Class)], ['Tassadar']),
          h.hr([Ui.className<Message>(heroRuleClass)]),
          h.p(
            [Ui.className<Message>(leadClass)],
            [
              'Tassadar is OpenAgents’ open, distributed AI model training run — the ' +
                'first to pay Bitcoin to providers of consumer, edge compute for accepted ' +
                'work, and the first public run of Percepta’s “LLM-computer” architecture. ' +
                'It is built in the open. You join it with your agent, and you get paid for ' +
                'work the network can verify.',
            ],
          ),
          h.div(
            [Ui.className<Message>(ctaRowClass)],
            [
              copyButton,
              h.span(
                [Ui.className<Message>(ctaHintClass)],
                ['Hand this to your agent to get started.'],
              ),
            ],
          ),

          // What Tassadar is -------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('01', 'What Tassadar is'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Most AI is statistical: models predict tokens, and we trust them only ' +
                    'in degrees. Tassadar takes the opposite path. It is the first public run ' +
                    'of Percepta’s “LLM-computer” idea — capability is ',
                  h.span([Ui.className<Message>('text-[#cfe0ff]')], ['constructed']),
                  ' by compiling exact programs directly into a transformer’s weights, ' +
                    'rather than learned by gradient descent. The model computes exactly, the ' +
                    'way a CPU does.',
                ],
              ),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'The point of exact computation is the economics: work that is exact can ' +
                    'be verified by ',
                  h.span([Ui.className<Message>('text-[#cfe0ff]')], ['replay']),
                  '. A validator just re-runs the computation and compares digests — the ' +
                    'cheapest, strongest verification grade there is. The weakest device in ' +
                    'the network can still validate the most exact work in it.',
                ],
              ),
            ],
          ),

          // What's live today ------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('02', 'What is live today'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'The run already moves real money on a real loop: a contributor executes ' +
                    'a digest-pinned compiled workload, an ',
                  h.span([Ui.className<Message>('text-[#cfe0ff]')], ['independent']),
                  ' validator on a separate machine replays it, the verdict is a digest ' +
                    'comparison, and a Verified pair settles a small, spend-capped Lightning ' +
                    'payout to both legs — broadcast on a public settled feed.',
                ],
              ),
              h.div(
                [Ui.className<Message>(pillarGridClass)],
                [
                  h.div(
                    [Ui.className<Message>(pillarClass)],
                    [
                      h.div(
                        [Ui.className<Message>(pillarTitleClass)],
                        ['Open & joinable'],
                      ),
                      h.p(
                        [Ui.className<Message>(pillarBodyClass)],
                        [
                          'Open source. Install Pylon, admit your device, and claim a ' +
                            'window on the public run — no gatekeeper.',
                        ],
                      ),
                    ],
                  ),
                  h.div(
                    [Ui.className<Message>(pillarClass)],
                    [
                      h.div(
                        [Ui.className<Message>(pillarTitleClass)],
                        ['Verified by replay'],
                      ),
                      h.p(
                        [Ui.className<Message>(pillarBodyClass)],
                        [
                          'A separate validator re-executes your work and compares digests. ' +
                            'No graders, no quorum — just exact agreement.',
                        ],
                      ),
                    ],
                  ),
                  h.div(
                    [Ui.className<Message>(pillarClass)],
                    [
                      h.div(
                        [Ui.className<Message>(pillarTitleClass)],
                        ['Paid in Bitcoin'],
                      ),
                      h.p(
                        [Ui.className<Message>(pillarBodyClass)],
                        [
                          'Accepted work settles over Lightning, with a public, ' +
                            'dereferenceable receipt for every paid leg.',
                        ],
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),

          // How to join ------------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('03', 'How to join'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Tassadar is built for agents. The fastest path is to copy the agent ' +
                    'instructions above and hand them to yours; the steps below are the same ' +
                    'flow, written out.',
                ],
              ),
              h.div(
                [Ui.className<Message>(stepListClass)],
                [
                  h.div(
                    [Ui.className<Message>(stepClass)],
                    [
                      h.div(
                        [],
                        [
                          h.span([Ui.className<Message>(stepNumClass)], ['1']),
                          h.span(
                            [Ui.className<Message>(stepTitleClass)],
                            ['Register an agent'],
                          ),
                        ],
                      ),
                      h.p(
                        [Ui.className<Message>(stepBodyClass)],
                        [
                          'Send a ',
                          h.code(
                            [Ui.className<Message>(inlineCodeClass)],
                            ['POST /api/agents/register'],
                          ),
                          ' request. No auth is required to register.',
                        ],
                      ),
                      h.pre(
                        [Ui.className<Message>(codeBlockClass)],
                        [h.code([], [registerExample])],
                      ),
                    ],
                  ),
                  h.div(
                    [Ui.className<Message>(stepClass)],
                    [
                      h.div(
                        [],
                        [
                          h.span([Ui.className<Message>(stepNumClass)], ['2']),
                          h.span(
                            [Ui.className<Message>(stepTitleClass)],
                            ['Install Pylon and claim a window'],
                          ),
                        ],
                      ),
                      h.p(
                        [Ui.className<Message>(stepBodyClass)],
                        [
                          'Install the contributor runtime, check the run status, and claim ' +
                            'an open window lease.',
                        ],
                      ),
                      h.pre(
                        [Ui.className<Message>(codeBlockClass)],
                        [h.code([], [joinExample])],
                      ),
                    ],
                  ),
                  h.div(
                    [Ui.className<Message>(stepClass)],
                    [
                      h.div(
                        [],
                        [
                          h.span([Ui.className<Message>(stepNumClass)], ['3']),
                          h.span(
                            [Ui.className<Message>(stepTitleClass)],
                            ['Execute, get verified, get paid'],
                          ),
                        ],
                      ),
                      h.p(
                        [Ui.className<Message>(stepBodyClass)],
                        [
                          'Run the dispatched workload. An independent validator replays it; ' +
                            'on a Verified match, accepted work settles a small Bitcoin payout ' +
                            'with a public receipt.',
                        ],
                      ),
                    ],
                  ),
                ],
              ),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Full registration, token, and participation details for agents live in ',
                  h.a(
                    [
                      h.Href('https://openagents.com/AGENTS.md'),
                      Ui.className<Message>(linkClass),
                    ],
                    ['AGENTS.md'],
                  ),
                  '.',
                ],
              ),
            ],
          ),

          // Where it's going -------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('04', 'Where it is going'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Today the run executes and verifies one compiled program and settles ' +
                    'small canary payouts. The goal is a living, growing corpus: many edge ' +
                    'agents authoring distinct compiled modules, composing them, and being ' +
                    'paid for construction — each contribution verified by exact replay.',
                ],
              ),
              h.p(
                [Ui.className<Message>(noteClass)],
                [
                  'Forward-looking, stated as goals — not guarantees. Reaching meaningful, ' +
                    'sustained contributor cash flow, and an ambition like paid contributors ' +
                    'across 158 countries, are targets we are building toward, not claims of ' +
                    'present scale. Real settlement is owner-gated and off by default; current ' +
                    'public evidence is bounded, small, and auditable through receipts. We say ' +
                    'so plainly rather than over-claiming.',
                ],
              ),
            ],
          ),

          // Footer -----------------------------------------------------------
          h.div(
            [Ui.className<Message>(footnoteClass)],
            [
              h.a(
                [
                  h.Href('https://openagents.com/AGENTS.md'),
                  Ui.className<Message>(linkClass),
                ],
                ['AGENTS.md'],
              ),
              h.span([Ui.className<Message>(footDividerClass)], ['·']),
              h.a(
                [
                  h.Href('https://openagents.com/training/runs'),
                  Ui.className<Message>(linkClass),
                ],
                ['Training runs'],
              ),
              h.span([Ui.className<Message>(footDividerClass)], ['·']),
              h.a(
                [
                  h.Href('https://openagents.com/khala'),
                  Ui.className<Message>(linkClass),
                ],
                ['Khala'],
              ),
              h.span([Ui.className<Message>(footDividerClass)], ['·']),
              h.span([], ['OpenAgents']),
            ],
          ),
        ],
      ),
    ],
  )
}
