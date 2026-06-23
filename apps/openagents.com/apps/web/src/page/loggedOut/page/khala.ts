import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import { ClickedExitKhala } from '../message'

// Public `/khala` surface: a readable, scrollable explainer for Khala, the
// OpenAgents OpenAI-compatible inference endpoint. This is a public content
// page rendered in the OpenAgents house style — StarCraft-Protoss energy: a
// dark void lit by glowing blue psionic energy, precise high-craft, technical
// mono typography (Berkeley Mono). The page sits transparently over the
// persistent 3D pylon scene (dimmed by the shared 75%-black scrim in
// persistentScene), with the long-form copy on a near-black, blue-tinted panel
// that lets a breath of the scene bleed through. See root DESIGN.md.
//
// Truthfulness rules (grounded in docs/inference/khala.md, AGENTS.md, and the
// live gateway in workers/api/src/inference):
//   - First-person plural "We are Khala". Never name an underlying provider.
//   - Only the two live model ids: `openagents/khala-mini`, `openagents/khala-code`.
//   - Base URL `https://openagents.com/v1`, OpenAI-compatible `/chat/completions`,
//     streaming via SSE (`"stream": true`).
//   - API keys come from the agent registration flow (`POST /api/agents/register`
//     -> `oa_agent_...` token, used as `Authorization: Bearer`).
//   - Verified work / receipts / credits + card/Bitcoin are framed honestly,
//     without promising capabilities we do not ship.

// --- design tokens (Protoss house style — see root DESIGN.md) -----------------

// Transparent background: the page sits over the persistent 3D pylon scene
// (dimmed by the shared 75%-black scrim in persistentScene), so the canvas
// stays visible on /khala just like /landing instead of an opaque page bg.
const pageClass =
  'relative min-h-screen min-h-dvh w-full overflow-y-auto font-mono text-[#f1efe8] antialiased'

// Back-to-home control: glowing-blue Protoss button on dark glass, fixed
// top-left so it is always reachable while scrolling. Emits ClickedExitKhala
// (wired in message.ts + update.ts -> NavigateToLanding, continuous camera
// flight back). No bounce; ease-out glow on hover.
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

// The long-form copy sits on its own near-black, blue-tinted panel (~92% so a
// breath of the 3D scene still bleeds through) with a faint energy bloom along
// its edges. Centered column with breathing room where the scene shows.
const containerClass =
  'khala-panel relative mx-auto my-12 w-[min(100%,880px)] overflow-hidden rounded-2xl ' +
  'bg-[#0a0e14]/92 px-6 py-16 ring-1 ring-[#3a7bff]/15 sm:my-16 sm:px-12 sm:py-20'

// Hero ------------------------------------------------------------------------
const eyebrowClass =
  'inline-flex items-center gap-2 font-mono text-[0.7rem] font-semibold uppercase ' +
  'tracking-[0.32em] text-[#8fb6ff]'

const eyebrowDotClass =
  'h-1.5 w-1.5 rounded-full bg-[#4fd0ff] khala-pulse'

const h1Class =
  'mt-6 font-mono text-[clamp(3rem,9vw,5.5rem)] font-bold leading-[0.95] tracking-[-0.04em] ' +
  'text-white text-balance'

const heroRuleClass = 'khala-rule mt-7 w-40'

const leadClass =
  'mt-7 max-w-[68ch] font-mono text-lg leading-relaxed text-[#d2dbe6] text-pretty'

// Sections --------------------------------------------------------------------
const sectionClass = 'mt-20 first:mt-0'

// Energized divider that opens each section (skipped on the first/hero section).
const sectionDividerClass = 'khala-rule mb-14 w-full'

const sectionHeadClass = 'flex items-center gap-4'

// Glowing-blue index marker replaces a per-section eyebrow (avoids AI eyebrow
// grammar; the number carries the section rhythm honestly).
const sectionIndexClass =
  'khala-index inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ' +
  'border border-[#3a7bff]/40 bg-[#0c1320] font-mono text-sm font-semibold text-[#7fc4ff]'

const sectionHeadingClass =
  'font-mono text-3xl font-bold tracking-[-0.02em] text-white sm:text-4xl'

const bodyClass =
  'mt-5 max-w-[68ch] font-mono text-base leading-relaxed text-[#c9d2dd] text-pretty'

const subHeadingClass =
  'mt-9 font-mono text-sm font-semibold uppercase tracking-[0.18em] text-[#8fb6ff]'

// Model cards -----------------------------------------------------------------
const cardGridClass = 'mt-7 grid gap-4 sm:grid-cols-2'

const cardClass =
  'group rounded-xl border border-[#1d2733] bg-[#0e141d] p-6 leading-relaxed ' +
  'transition-all duration-300 ease-out hover:border-[#3a7bff]/55 hover:bg-[#101926] ' +
  'hover:khala-glow motion-reduce:transition-none'

const cardTitleClass = 'font-mono text-sm font-bold tracking-[-0.01em] text-[#7fc4ff]'

const cardBodyClass = 'mt-3 font-mono text-sm leading-relaxed text-[#c2ccd8]'

// Code ------------------------------------------------------------------------
const codeBlockClass =
  'mt-5 overflow-x-auto rounded-xl border border-[#1d2733] bg-[#06090e] p-5 font-mono ' +
  'text-[13px] leading-relaxed text-[#d7e2f0] ring-1 ring-inset ring-[#3a7bff]/10'

const inlineCodeClass =
  'rounded bg-[#101926] px-1.5 py-0.5 font-mono text-[0.9em] text-[#cfe0ff] ' +
  'ring-1 ring-inset ring-[#3a7bff]/15'

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

// Misc ------------------------------------------------------------------------
const linkClass =
  'font-medium text-[#7fc4ff] underline decoration-[#3a7bff]/50 underline-offset-2 ' +
  'transition-colors hover:text-[#4fd0ff] hover:decoration-[#4fd0ff]/70 ' +
  'motion-reduce:transition-none'

const noteClass =
  'mt-7 rounded-xl border border-[#3a7bff]/20 bg-[#080d15] p-6 font-mono text-sm ' +
  'leading-relaxed text-[#aeb9c6] ring-1 ring-inset ring-[#3a7bff]/10'

const footnoteClass =
  'mt-20 flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-[#1d2733] pt-8 ' +
  'font-mono text-sm text-[#7e8a98]'

const footDividerClass = 'text-[#3a7bff]/40'

// --- code samples (kept as plain strings so the page is purely declarative) ---

const curlExample = `curl https://openagents.com/v1/chat/completions \\
  -H "Authorization: Bearer $OA_AGENT_TOKEN" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "openagents/khala-mini",
    "messages": [
      { "role": "user", "content": "Say hello from Khala." }
    ]
  }'`

const sdkExample = `from openai import OpenAI

client = OpenAI(
    base_url="https://openagents.com/v1",
    api_key="oa_agent_...",  # your OpenAgents agent token
)

response = client.chat.completions.create(
    model="openagents/khala-code",
    messages=[{"role": "user", "content": "Write a failing test, then make it pass."}],
)

print(response.choices[0].message.content)`

const streamExample = `# Streaming is OpenAI-compatible: set "stream": true and read SSE chunks.
stream = client.chat.completions.create(
    model="openagents/khala-mini",
    messages=[{"role": "user", "content": "Stream a short poem."}],
    stream=True,
)
for chunk in stream:
    print(chunk.choices[0].delta.content or "", end="")`

const registerExample = `curl https://openagents.com/api/agents/register \\
  -H "Content-Type: application/json" \\
  -d '{ "agentName": "my-agent" }'
# -> { "agentToken": "oa_agent_..." }`

// --- view ---------------------------------------------------------------------

export const view = (): Html => {
  const h = html<Message>()

  const code = (label: string, body: string): Html =>
    h.div(
      [],
      [
        h.div([Ui.className<Message>(subHeadingClass)], [label]),
        h.pre([Ui.className<Message>(codeBlockClass)], [h.code([], [body])]),
      ],
    )

  // A section opens with an energized divider, a glowing index marker, and a
  // mono heading. The index carries the rhythm honestly (it is a sequence).
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
          h.DataAttribute('khala-back', 'home'),
          Ui.className<Message>(backButtonClass),
        ],
        [
          h.span([Ui.className<Message>(backArrowClass)], ['←']),
          h.span([], ['OpenAgents']),
        ],
      ),
    ],
  )

  return h.div(
    [
      h.DataAttribute('route', 'khala'),
      Ui.className<Message>(pageClass),
    ],
    [
      backButton,
      h.main(
        [h.AriaLabel('Khala'), Ui.className<Message>(containerClass)],
        [
          // Hero -------------------------------------------------------------
          h.div(
            [Ui.className<Message>(eyebrowClass)],
            [
              h.span([Ui.className<Message>(eyebrowDotClass)], []),
              h.span([], ['OpenAgents Inference']),
            ],
          ),
          h.h1([Ui.className<Message>(h1Class)], ['Khala']),
          h.hr([Ui.className<Message>(heroRuleClass)]),
          h.p(
            [Ui.className<Message>(leadClass)],
            [
              'We are Khala — one OpenAI-compatible endpoint over a network of agents. ' +
                'You call a single API. Underneath, requests are routed and orchestrated across a pool ' +
                'of models, tools, and validators, with verified work and public receipts.',
            ],
          ),

          // What is Khala ----------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('01', 'What is Khala'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Khala is a single inference endpoint that behaves like one model but is an agent ' +
                    'network underneath. To your code it is just an OpenAI-compatible Chat Completions API. ' +
                    'Behind that one surface, Khala routes each request across a pool of models and ' +
                    'validators, picks a lane, runs the work, and returns the answer — plus a receipt ' +
                    'describing what actually happened.',
                ],
              ),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'The contract is always "one Khala endpoint." Each response discloses which concrete ' +
                    'worker served it, so you can see the route without changing how you call the API.',
                ],
              ),
            ],
          ),

          // Models -----------------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('02', 'Models'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Two models are live today. Pass the model id in the standard OpenAI ',
                  h.code([Ui.className<Message>(inlineCodeClass)], ['model']),
                  ' field.',
                ],
              ),
              h.div(
                [Ui.className<Message>(cardGridClass)],
                [
                  h.div(
                    [Ui.className<Message>(cardClass)],
                    [
                      h.div(
                        [Ui.className<Message>(cardTitleClass)],
                        ['openagents/khala-mini'],
                      ),
                      h.p(
                        [Ui.className<Message>(cardBodyClass)],
                        [
                          'The cheap default. A cheapest-viable router over the pool — a good fit for ' +
                            'agents, chat, and general text work.',
                        ],
                      ),
                    ],
                  ),
                  h.div(
                    [Ui.className<Message>(cardClass)],
                    [
                      h.div(
                        [Ui.className<Message>(cardTitleClass)],
                        ['openagents/khala-code'],
                      ),
                      h.p(
                        [Ui.className<Message>(cardBodyClass)],
                        [
                          'Coding-optimized. Can run tests and verification commands and returns a ' +
                            'verification verdict in the receipt.',
                        ],
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),

          // How to use it ----------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('03', 'How to use it'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Point any OpenAI-compatible client at the base URL ',
                  h.code(
                    [Ui.className<Message>(inlineCodeClass)],
                    ['https://openagents.com/v1'],
                  ),
                  ' and call ',
                  h.code(
                    [Ui.className<Message>(inlineCodeClass)],
                    ['/chat/completions'],
                  ),
                  ' with your agent token as a bearer credential.',
                ],
              ),
              code('curl', curlExample),
              code('OpenAI SDK (Python)', sdkExample),
              code('Streaming', streamExample),
              h.p(
                [Ui.className<Message>(noteClass)],
                [
                  'Streaming is OpenAI-compatible: set ',
                  h.code(
                    [Ui.className<Message>(inlineCodeClass)],
                    ['"stream": true'],
                  ),
                  ' and read Server-Sent Events. You get incremental ',
                  h.code(
                    [Ui.className<Message>(inlineCodeClass)],
                    ['chat.completion.chunk'],
                  ),
                  ' deltas, and the final chunk carries an ',
                  h.code([Ui.className<Message>(inlineCodeClass)], ['openagents']),
                  ' block with the route and receipt. Clients that do not recognize that ' +
                    'block simply ignore it, so existing OpenAI SDKs work unchanged.',
                ],
              ),
            ],
          ),

          // Get an API key ---------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('04', 'Get an API key'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Khala uses OpenAgents agent tokens. Register an agent to get one — no auth required ' +
                    'to register.',
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
                          ' request with an agent name.',
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
                            ['Keep your token'],
                          ),
                        ],
                      ),
                      h.p(
                        [Ui.className<Message>(stepBodyClass)],
                        [
                          'The response returns an ',
                          h.code(
                            [Ui.className<Message>(inlineCodeClass)],
                            ['oa_agent_...'],
                          ),
                          ' token. Store it as a secret — it is your API key.',
                        ],
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
                            ['Call Khala'],
                          ),
                        ],
                      ),
                      h.p(
                        [Ui.className<Message>(stepBodyClass)],
                        [
                          'Pass the token as ',
                          h.code(
                            [Ui.className<Message>(inlineCodeClass)],
                            ['Authorization: Bearer oa_agent_...'],
                          ),
                          ' on every request to ',
                          h.code(
                            [Ui.className<Message>(inlineCodeClass)],
                            ['https://openagents.com/v1'],
                          ),
                          '.',
                        ],
                      ),
                    ],
                  ),
                ],
              ),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Full registration and token details for agents live in ',
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

          // Credits & pricing ------------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('05', 'Credits & pricing'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Khala is pay-per-call and per-token. Usage is billed in credits and metered from ' +
                    'the receipt, never from an estimate. Fund your account with a card or with Bitcoin — ' +
                    'Bitcoin carries a small discount funded by the card-processing fees we save.',
                ],
              ),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'Every call returns a dereferenceable receipt with token counts and metered cost, so ' +
                    'spend is auditable rather than opaque.',
                ],
              ),
            ],
          ),

          // Verified, with receipts ------------------------------------------
          h.section(
            [Ui.className<Message>(sectionClass)],
            [
              sectionHead('06', 'Verified, with receipts'),
              h.p(
                [Ui.className<Message>(bodyClass)],
                [
                  'What sets Khala apart from a plain router is evidence. Every response carries an ',
                  h.code([Ui.className<Message>(inlineCodeClass)], ['openagents']),
                  ' block: which lane served the request, the metered cost, and a receipt id you ' +
                    'can dereference. For ',
                  h.code(
                    [Ui.className<Message>(inlineCodeClass)],
                    ['openagents/khala-code'],
                  ),
                  ', the receipt records a verification verdict — for example, that tests passed.',
                ],
              ),
              h.p(
                [Ui.className<Message>(noteClass)],
                [
                  'Khala is built in the open and shipping in phases. Verification classes and public ' +
                    'usage receipts are live now; learned coordination, contributor worker payouts, and ' +
                    'machine-payable settlement are on the roadmap. We say so plainly rather than ' +
                    'over-claiming.',
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
                  h.Href('https://openagents.com/docs/product-promises'),
                  Ui.className<Message>(linkClass),
                ],
                ['Product promises'],
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
