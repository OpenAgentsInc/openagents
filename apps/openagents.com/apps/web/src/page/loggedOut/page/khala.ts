import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'

// Public `/khala` surface: a readable, scrollable explainer for Khala, the
// OpenAgents OpenAI-compatible inference endpoint. This is a normal public
// content page (not the chrome-free landing/moksha canvas): a dark, legible
// theme that matches the site palette, with sections for what Khala is, the
// models, how to call it, how to get an API key, and credits/pricing.
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

// --- design tokens (match the site dark public palette) -----------------------

const pageClass =
  'min-h-screen min-h-dvh w-full overflow-y-auto bg-[#0c0f13] text-[#f1efe8] antialiased'

const containerClass = 'mx-auto w-[min(100%,860px)] px-5 py-16 sm:py-20'

const eyebrowClass =
  'text-xs font-semibold uppercase tracking-[0.22em] text-[#8fb6ff]'

const h1Class =
  'mt-4 text-4xl font-semibold tracking-tight text-white sm:text-5xl'

const leadClass = 'mt-5 max-w-2xl text-lg leading-relaxed text-[#c9d2dd]'

const sectionClass = 'mt-14 border-t border-[#1d2530] pt-12 first:mt-0'

const sectionHeadingClass =
  'text-2xl font-semibold tracking-tight text-white sm:text-3xl'

const bodyClass = 'mt-4 max-w-2xl text-base leading-relaxed text-[#c9d2dd]'

const subHeadingClass = 'mt-8 text-lg font-semibold text-white'

const cardGridClass = 'mt-6 grid gap-4 sm:grid-cols-2'

const cardClass =
  'rounded-xl border border-[#1d2530] bg-[#11161d] p-5 leading-relaxed'

const cardTitleClass = 'font-mono text-sm font-semibold text-[#8fb6ff]'

const cardBodyClass = 'mt-2 text-sm leading-relaxed text-[#c9d2dd]'

const codeBlockClass =
  'mt-5 overflow-x-auto rounded-xl border border-[#1d2530] bg-[#0a0d12] p-5 font-mono text-[13px] leading-relaxed text-[#d7e2f0]'

const inlineCodeClass =
  'rounded bg-[#11161d] px-1.5 py-0.5 font-mono text-[0.9em] text-[#cfe0ff]'

const stepListClass = 'mt-6 grid gap-4'

const stepClass = 'rounded-xl border border-[#1d2530] bg-[#11161d] p-5'

const stepNumClass =
  'inline-flex h-6 w-6 items-center justify-center rounded-full bg-[#1f3354] font-mono text-xs font-semibold text-[#bcd4ff]'

const stepTitleClass = 'ml-3 text-base font-semibold text-white'

const stepBodyClass = 'mt-2 text-sm leading-relaxed text-[#c9d2dd]'

const linkClass =
  'font-medium text-[#8fb6ff] underline decoration-[#345] underline-offset-2 hover:text-white'

const noteClass =
  'mt-6 rounded-xl border border-[#243043] bg-[#0e141c] p-5 text-sm leading-relaxed text-[#aeb9c6]'

const footnoteClass = 'mt-16 border-t border-[#1d2530] pt-8 text-sm text-[#7e8a98]'

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

  return h.div(
    [
      h.DataAttribute('route', 'khala'),
      Ui.className<Message>(pageClass),
    ],
    [
      h.main(
        [h.AriaLabel('Khala'), Ui.className<Message>(containerClass)],
        [
          // Hero -------------------------------------------------------------
          h.div(
            [Ui.className<Message>(eyebrowClass)],
            ['OpenAgents Inference'],
          ),
          h.h1([Ui.className<Message>(h1Class)], ['Khala']),
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
              h.h2(
                [Ui.className<Message>(sectionHeadingClass)],
                ['What is Khala'],
              ),
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
              h.h2([Ui.className<Message>(sectionHeadingClass)], ['Models']),
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
              h.h2(
                [Ui.className<Message>(sectionHeadingClass)],
                ['How to use it'],
              ),
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
              h.h2(
                [Ui.className<Message>(sectionHeadingClass)],
                ['Get an API key'],
              ),
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
              h.h2(
                [Ui.className<Message>(sectionHeadingClass)],
                ['Credits & pricing'],
              ),
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
              h.h2(
                [Ui.className<Message>(sectionHeadingClass)],
                ['Verified, with receipts'],
              ),
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
              '  ·  ',
              h.a(
                [
                  h.Href('https://openagents.com/docs/product-promises'),
                  Ui.className<Message>(linkClass),
                ],
                ['Product promises'],
              ),
              '  ·  OpenAgents',
            ],
          ),
        ],
      ),
    ],
  )
}
