// Generic Khala chat — the page view for the `/khala` chat box.
//
// `/khala` shows NOTHING on screen except the chat box and the 3D pylon scene
// (owned by `page/loggedOut/page/persistentScene.ts` at the `khala` pose). This
// module renders ONLY:
//   - the chat box overlay: a scrollable message thread + a pinned composer,
//     reusing the SAME `@openagentsinc/ui` AI Elements the `/autopilot` chat uses
//     (the `message`/`response` markdown renderer + the `prompt-input` composer),
//     wired to the generic streaming chat (`POST /api/khala/chat`);
//   - a small, unobtrusive "What is Khala?" trigger in the corner that opens an
//     info popup overlay (modal) with the condensed Khala basics.
//
// The long-form explainer that used to live inline on `/khala` is GONE; its
// content is condensed into the info popup. This is GENERIC Khala — not the
// Autopilot Concierge / onboarding intake agent. Dark / mono / Khala-blue over
// the dimmed 3D scene, reduced-motion safe.

import { Option } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { AiElements, eyebrowClass } from '@openagentsinc/ui'

import { iconView } from '../../icon'
import * as Ui from '../../ui'
import type { KhalaChatModel } from './flow'

// Public data attributes so tests / captures / the scroll command can locate the
// chat surfaces.
export const KHALA_CHAT_ROOT_ATTR = 'khala-chat'
export const KHALA_CHAT_TRANSCRIPT_ATTR = 'khala-chat-transcript'
export const KHALA_CHAT_COMPOSER_ATTR = 'khala-chat-composer'
export const KHALA_CHAT_THREAD_END_ATTR = 'khala-chat-thread-end'
export const KHALA_CHAT_INFO_TRIGGER_ATTR = 'khala-chat-info-trigger'
export const KHALA_CHAT_INFO_DIALOG_ATTR = 'khala-chat-info-dialog'
// The selector the scroll-to-end command targets.
export const KHALA_CHAT_THREAD_END_SELECTOR = `[data-${KHALA_CHAT_THREAD_END_ATTR}="true"]`

const KHALA_HEADING = 'Khala'

// The message hooks the page needs. Threading these in (rather than importing a
// concrete page `Message`) keeps this view decoupled and unit-testable.
export type KhalaChatViewActions<Message> = Readonly<{
  updatedComposer: (value: string) => Message
  submittedTurn: () => Message
  openedInfo: () => Message
  closedInfo: () => Message
}>

// TRANSCRIPT --------------------------------------------------------------

// One transcript turn. The assistant body renders as Markdown through the
// centralized `response` element; the user body is plain text.
const transcriptTurnView = <Message>(turn: {
  role: 'user' | 'assistant'
  content: string
}): Html =>
  turn.role === 'assistant'
    ? AiElements.message<Message>({
        props: { role: 'assistant', author: KHALA_HEADING },
        markdown: turn.content,
      })
    : AiElements.message<Message>({
        props: { role: 'user', body: turn.content, author: 'You' },
      })

// The in-flight streaming assistant bubble: the partial reply rendered as
// progressive Markdown with a live typing cursor. Only present while streaming.
const streamingTurnView = <Message>(partial: string): Html =>
  AiElements.message<Message>({
    props: { role: 'assistant', author: KHALA_HEADING },
    markdown: partial,
    streaming: true,
  })

// The conversation thread: the running transcript plus the in-flight streaming
// bubble, and a bottom sentinel the scroll-to-end command and native scroll
// anchoring both target. An empty thread shows a calm starter hint.
const transcriptView = <Message>(model: KhalaChatModel): Html => {
  const h = html<Message>()

  const isEmpty =
    model.transcript.length === 0 && model.streamingReply === null

  return h.div(
    [
      h.DataAttribute(KHALA_CHAT_TRANSCRIPT_ATTR, ''),
      Ui.className<Message>('grid gap-3'),
    ],
    [
      isEmpty
        ? h.p(
            [
              Ui.className<Message>(
                'm-0 text-[0.8125rem] leading-[1.5] text-white/45',
              ),
            ],
            ['Ask Khala what it can do.'],
          )
        : h.empty,
      ...model.transcript.map(turn => transcriptTurnView<Message>(turn)),
      model.streamingReply === null
        ? h.empty
        : streamingTurnView<Message>(model.streamingReply),
      h.div(
        [
          h.DataAttribute(KHALA_CHAT_THREAD_END_ATTR, 'true'),
          Ui.className<Message>('h-px w-full [overflow-anchor:auto]'),
        ],
        [],
      ),
    ],
  )
}

// COMPOSER ----------------------------------------------------------------

const composerView = <Message>(
  model: KhalaChatModel,
  actions: KhalaChatViewActions<Message>,
): Html => {
  const h = html<Message>()

  const inFlight =
    model.status === 'submitting' || model.status === 'streaming'

  const status =
    model.status === 'streaming'
      ? ('streaming' as const)
      : model.status === 'submitting'
        ? ('submitted' as const)
        : model.status === 'error'
          ? ('error' as const)
          : ('ready' as const)

  const submitAttrs: ReadonlyArray<Attribute<Message>> =
    inFlight || model.composerDraft.trim() === '' ? [h.Disabled(true)] : []

  return h.div(
    [
      h.DataAttribute(KHALA_CHAT_COMPOSER_ATTR, ''),
      Ui.className<Message>('grid gap-2'),
    ],
    [
      model.status === 'error' && model.errorReason !== null
        ? h.p(
            [
              h.Role('alert'),
              Ui.className<Message>('m-0 text-[0.75rem] text-[#ff6f00]'),
            ],
            [model.errorReason],
          )
        : h.empty,
      AiElements.promptInput<Message>({
        props: {
          name: 'khala-chat-message',
          placeholder: 'Message Khala…',
          value: model.composerDraft,
          status,
          submitLabel: 'Send',
        },
        formAttrs: [h.OnSubmit(actions.submittedTurn())],
        textareaAttrs: [
          h.Value(model.composerDraft),
          h.OnInput(value => actions.updatedComposer(value)),
          h.AriaLabel('Message Khala'),
          // Enter submits the turn; Shift+Enter inserts a newline.
          h.OnKeyDownPreventDefault((key, modifiers) =>
            key === 'Enter' &&
            !modifiers.shiftKey &&
            !inFlight &&
            model.composerDraft.trim() !== ''
              ? Option.some(actions.submittedTurn())
              : Option.none(),
          ),
        ],
        submitAttrs,
      }),
    ],
  )
}

// INFO POPUP --------------------------------------------------------------

const infoTriggerClass =
  'khala-focus pointer-events-auto inline-flex items-center gap-1.5 rounded-full ' +
  'border border-[#3a7bff]/40 bg-[#070b12]/80 px-3 py-1.5 font-mono text-[0.6875rem] ' +
  'font-semibold uppercase tracking-[0.18em] text-[#bcd4ff] backdrop-blur-md ' +
  'transition-colors duration-300 ease-out hover:border-[#4fd0ff]/80 hover:text-white ' +
  'motion-reduce:transition-none'

const infoInlineCodeClass =
  'rounded bg-[#101926] px-1.5 py-0.5 font-mono text-[0.85em] text-[#cfe0ff] ' +
  'ring-1 ring-inset ring-[#3a7bff]/15'

// One condensed info row: a label + a body. Bodies are truthful per
// docs/khala/khala.md (first-person "We are Khala", never a provider).
const infoRow = <Message>(label: string, body: ReadonlyArray<Html | string>): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>('grid gap-1.5')],
    [
      h.div(
        [
          Ui.className<Message>(
            'font-mono text-[0.6875rem] font-semibold uppercase tracking-[0.18em] text-[#8fb6ff]',
          ),
        ],
        [label],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.8125rem] leading-[1.55] text-[#c9d2dd]',
          ),
        ],
        body,
      ),
    ],
  )
}

// The "What is Khala?" info popup overlay (modal). Rendered ONLY when
// `model.infoOpen` is true. It escapes the chat-box stacking context (mounted at
// the overlay root, z above the chat) and is closeable by the Close button, a
// backdrop click, or Escape. The backdrop and the dialog panel are SIBLINGS (the
// backdrop sits behind the panel and owns the close-on-click), so a click on the
// panel never reaches the backdrop — no stop-propagation needed. Reduced-motion
// safe (no entrance animation needed).
const infoPopup = <Message>(
  actions: KhalaChatViewActions<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('khala-chat-info-overlay', ''),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-30 flex items-center justify-center px-4 py-8',
      ),
    ],
    [
      // Backdrop: a full-screen sibling BEHIND the panel that dims the scene and
      // closes the popup on click.
      h.button(
        [
          h.Type('button'),
          h.DataAttribute('khala-chat-info-backdrop', ''),
          h.OnClick(actions.closedInfo()),
          h.AriaLabel('Close'),
          h.Tabindex(-1),
          Ui.className<Message>(
            'pointer-events-auto absolute inset-0 z-0 cursor-default bg-black/70',
          ),
        ],
        [],
      ),
      h.div(
        [
          h.DataAttribute(KHALA_CHAT_INFO_DIALOG_ATTR, ''),
          h.Role('dialog'),
          h.AriaModal(true),
          h.AriaLabel('What is Khala?'),
          h.Tabindex(-1),
          // Escape closes the dialog.
          h.OnKeyDownPreventDefault((key: string) =>
            key === 'Escape'
              ? Option.some(actions.closedInfo())
              : Option.none(),
          ),
          Ui.className<Message>(
            'khala-glow pointer-events-auto relative z-10 grid max-h-full w-[min(100%,34rem)] gap-5 ' +
              'overflow-y-auto rounded-2xl border border-[#3a7bff]/30 bg-[#0a0e14]/95 p-6 ' +
              'backdrop-blur-md sm:p-8',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('flex items-start justify-between gap-4')],
            [
              h.div(
                [Ui.className<Message>('grid gap-1.5')],
                [
                  h.span(
                    [Ui.className<Message>(eyebrowClass)],
                    ['OpenAgents Inference'],
                  ),
                  h.h2(
                    [
                      Ui.className<Message>(
                        'm-0 font-mono text-2xl font-bold tracking-[-0.02em] text-white',
                      ),
                    ],
                    ['What is Khala?'],
                  ),
                ],
              ),
              h.button(
                [
                  h.Type('button'),
                  h.OnClick(actions.closedInfo()),
                  h.AriaLabel('Close'),
                  Ui.className<Message>(
                    'khala-focus pointer-events-auto inline-flex items-center justify-center rounded-full ' +
                      'border border-[#222] bg-[#070b12]/80 p-2 text-white/55 transition-colors ' +
                      'hover:border-[#3a7bff]/45 hover:text-white motion-reduce:transition-none',
                  ),
                ],
                [iconView<Message>('XXs', 'size-4')],
              ),
            ],
          ),
          infoRow<Message>('Khala', [
            'We are Khala — one OpenAI-compatible endpoint over a network of agents. ' +
              'You call a single API; underneath, requests are routed and orchestrated across a pool ' +
              'of models, tools, and validators, with receipt-backed disclosure about what happened.',
          ]),
          infoRow<Message>('Model', [
            'One public model id: ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['openagents/khala']),
            '. The orchestrator picks the backing lane; you buy the outcome.',
          ]),
          infoRow<Message>('API', [
            'OpenAI-compatible. Point any OpenAI client at the base URL ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['https://openagents.com/api/v1']),
            ' and call ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['/chat/completions']),
            '. Streaming works over standard Server-Sent Events (set ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['"stream": true']),
            ').',
          ]),
          infoRow<Message>('Free', [
            'Free to use, no signup. Mint a key with ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['POST /api/keys/free']),
            ', then send it as ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['Authorization: Bearer <token>']),
            '. Free quota: 200 requests/day, 200k tokens/day.',
          ]),
          h.p(
            [
              Ui.className<Message>(
                'm-0 border-t border-[#1d2733] pt-4 font-mono text-[0.75rem] leading-[1.5] text-[#7e8a98]',
              ),
            ],
            [
              'Full details for agents live at ',
              h.a(
                [
                  h.Href('https://openagents.com/AGENTS.md'),
                  Ui.className<Message>(
                    'text-[#7fc4ff] underline decoration-[#3a7bff]/50 underline-offset-2 hover:text-[#4fd0ff]',
                  ),
                ],
                ['AGENTS.md'],
              ),
              '.',
            ],
          ),
        ],
      ),
    ],
  )
}

// INSTRUCTIONS PAGE (the /khala overlay) ---------------------------------
//
// /khala renders a concise API-instructions panel over the dimmed scene. The
// generic chat box is intentionally NOT shown here yet (not ready); this is the
// AGENTS.md "Run inference" basics — base URL, single model, the free self-serve
// token, a copy-paste curl — so a visitor can start calling Khala immediately.
export const KHALA_INSTRUCTIONS_ATTR = 'khala-instructions'

const instructionsCurl =
  'KEY=$(curl -s -X POST https://openagents.com/api/keys/free | jq -r .credential.token)\n\n' +
  'curl https://openagents.com/api/v1/chat/completions \\\n' +
  '  -H "Authorization: Bearer $KEY" \\\n' +
  '  -H "Content-Type: application/json" \\\n' +
  '  -d \'{"model":"openagents/khala","messages":[{"role":"user","content":"hello"}]}\''

export const instructionsView = <Message>(): Html => {
  const h = html<Message>()
  return h.div(
    [
      h.DataAttribute(KHALA_INSTRUCTIONS_ATTR, ''),
      Ui.className<Message>(
        'pointer-events-auto absolute inset-0 z-20 flex items-start justify-center overflow-y-auto p-6 sm:items-center',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid w-full max-w-xl gap-5 rounded-xl border border-[#1d2733] ' +
              'bg-[#04070c]/90 p-7 shadow-2xl backdrop-blur-md',
          ),
        ],
        [
          h.div([Ui.className<Message>(eyebrowClass)], ['Research preview']),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 font-semibold text-white text-3xl tracking-tight',
              ),
            ],
            [KHALA_HEADING],
          ),
          infoRow<Message>('Khala', [
            'We are Khala — one OpenAI-compatible endpoint over a network of agents. ' +
              'You call a single API; underneath, requests are routed and orchestrated across a pool ' +
              'of models, tools, and validators, with receipt-backed disclosure about what happened.',
          ]),
          infoRow<Message>('Model', [
            'One public model id: ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['openagents/khala']),
            '. The orchestrator picks the backing lane; you buy the outcome.',
          ]),
          infoRow<Message>('API', [
            'OpenAI-compatible. Point any OpenAI client at the base URL ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['https://openagents.com/api/v1']),
            ' and call ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['/chat/completions']),
            '. Streaming works over standard Server-Sent Events (set ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['"stream": true']),
            ').',
          ]),
          infoRow<Message>('Free token', [
            'Free to use, no signup. Mint a key with ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['POST /api/keys/free']),
            ' (use the returned ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['credential.token']),
            '), then send it as ',
            h.code([Ui.className<Message>(infoInlineCodeClass)], ['Authorization: Bearer <token>']),
            '. Free quota: 200 requests/day, 200,000 tokens/day.',
          ]),
          h.div(
            [Ui.className<Message>('grid gap-1.5')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'font-mono text-[0.6875rem] font-semibold uppercase tracking-[0.18em] text-[#8fb6ff]',
                  ),
                ],
                ['Try it'],
              ),
              h.pre(
                [
                  Ui.className<Message>(
                    'm-0 overflow-x-auto rounded bg-[#101926] p-3 font-mono text-[0.72rem] ' +
                      'leading-[1.5] text-[#cfe0ff] ring-1 ring-inset ring-[#3a7bff]/15',
                  ),
                ],
                [instructionsCurl],
              ),
            ],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 border-t border-[#1d2733] pt-4 font-mono text-[0.75rem] leading-[1.5] text-[#7e8a98]',
              ),
            ],
            [
              'Full details for agents live at ',
              h.a(
                [
                  h.Href('https://openagents.com/AGENTS.md'),
                  Ui.className<Message>(
                    'text-[#7fc4ff] underline decoration-[#3a7bff]/50 underline-offset-2 hover:text-[#4fd0ff]',
                  ),
                ],
                ['AGENTS.md'],
              ),
              '.',
            ],
          ),
        ],
      ),
    ],
  )
}

// OVERLAY -----------------------------------------------------------------

// The chat overlay: the chat box floating over the dimmed scene, plus the small
// corner info trigger and (when open) the info popup. The composer is pinned
// BELOW the scroll region so it never scrolls away; only the thread scrolls
// internally.
export const overlayView = <Message>(
  model: KhalaChatModel,
  actions: KhalaChatViewActions<Message>,
): Html => {
  const h = html<Message>()

  const scrollRegion = h.div(
    [
      Ui.className<Message>(
        'oa-thread-scroll grid min-h-0 content-start gap-3 overflow-y-auto pr-1',
      ),
    ],
    [transcriptView<Message>(model)],
  )

  const infoTrigger = h.div(
    [Ui.className<Message>('pointer-events-none absolute right-4 top-4 z-20 sm:right-6 sm:top-6')],
    [
      h.button(
        [
          h.Type('button'),
          h.DataAttribute(KHALA_CHAT_INFO_TRIGGER_ATTR, ''),
          h.OnClick(actions.openedInfo()),
          h.AriaLabel('What is Khala?'),
          Ui.className<Message>(infoTriggerClass),
        ],
        [
          iconView<Message>('InfoCircle', 'size-3.5 text-[#4fd0ff]'),
          h.span([], ['What is Khala?']),
        ],
      ),
    ],
  )

  return h.div(
    [
      h.DataAttribute(KHALA_CHAT_ROOT_ATTR, ''),
      h.AriaLabel('Khala chat'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex items-stretch justify-center px-4 py-6 sm:py-10',
      ),
    ],
    [
      infoTrigger,
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-auto m-auto grid max-h-full w-[min(100%,48rem)] content-start gap-4 ' +
              'border border-[#3a7bff]/25 bg-black/55 p-4 backdrop-blur-md khala-glow sm:p-6',
          ),
        ],
        [scrollRegion, composerView<Message>(model, actions)],
      ),
      model.infoOpen ? infoPopup<Message>(actions) : h.empty,
    ],
  )
}
