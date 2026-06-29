import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

export type PylonCodexAssignmentStatusRouteLike = Readonly<{
  _tag: 'PylonCodexAssignmentStatus'
  assignmentRef: string
}>

const pageShellClass = 'min-h-screen bg-[#090a0c] text-[#f4f1e8]'

const shell = 'mx-auto grid min-h-screen w-full max-w-6xl gap-8 px-4 py-6 sm:px-6 lg:px-8'

const panel =
  'border border-white/10 bg-[#101216] p-4 shadow-xl shadow-black/20 sm:p-5'

const eyebrow =
  'm-0 text-[0.68rem] font-semibold uppercase leading-none tracking-[0.14em] text-emerald-200/70'

const titleClass =
  'm-0 max-w-3xl text-3xl font-semibold leading-tight text-white sm:text-4xl'

const bodyClass = 'm-0 max-w-3xl text-sm leading-6 text-white/62'

const commandClass =
  'overflow-x-auto border border-white/10 bg-black/45 p-3 text-xs leading-6 text-emerald-100/90'

const mutedCodeClass =
  'break-all rounded bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-white/82'

const statusStages = [
  ['request', 'Khala request emitted assignment ref'],
  ['run', 'Pylon run-no-spend claimed local Codex work'],
  ['status', 'Trace-status shows private chunks and lifecycle'],
  ['proof', 'Proof checklist validates exact owner-capacity evidence'],
] as const

const codeInline = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.code([Ui.className<Message>(mutedCodeClass)], [text])
}

const commandBlock = <Message>(command: string): Html => {
  const h = html<Message>()
  return h.pre(
    [Ui.className<Message>(commandClass)],
    [h.code([], [command])],
  )
}

const stage = <Message>(label: string, copy: string): Html => {
  const h = html<Message>()
  return h.li(
    [
      Ui.className<Message>(
        'grid gap-2 border border-white/10 bg-white/[0.035] p-3',
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            'font-mono text-[0.68rem] font-semibold uppercase leading-none tracking-[0.14em] text-sky-200/75',
          ),
        ],
        [label],
      ),
      h.span([Ui.className<Message>('text-sm leading-5 text-white/68')], [copy]),
    ],
  )
}

const statusCopy = (assignmentRef: string): readonly [string, string] => [
  `pylon khala status --assignment-ref ${JSON.stringify(assignmentRef)} --json`,
  `pylon khala proof ${JSON.stringify(assignmentRef)} --json`,
]

export const view = <Message>(
  route: PylonCodexAssignmentStatusRouteLike,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const [statusCommand, proofCommand] = statusCopy(route.assignmentRef)

  return h.div(
    [Ui.className<Message>(pageShellClass), h.DataAttribute('route', 'pylon-codex-assignment-status')],
    [
      PublicHeader.view(authState),
      h.main(
        [Ui.className<Message>(shell), h.AriaLabel('Pylon Codex assignment status')],
        [
          h.section(
            [
              Ui.className<Message>(
                'grid gap-5 border border-white/10 bg-[#0d0f13] p-5 sm:p-6',
              ),
            ],
            [
              h.p([Ui.className<Message>(eyebrow)], ['Owner capacity status']),
              h.h1([Ui.className<Message>(titleClass)], [
                'Pylon Codex assignment',
              ]),
              h.p([Ui.className<Message>(bodyClass)], [
                'This page is the stable operator surface for one Khala coding delegation. The live evidence remains owner-scoped: use an agent token through the CLI/API to read private trace chunks, final token usage, and proof metadata.',
              ]),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-2 border border-white/10 bg-black/25 p-3',
                  ),
                ],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        'text-[0.68rem] font-semibold uppercase leading-none tracking-[0.14em] text-white/42',
                      ),
                    ],
                    ['Assignment'],
                  ),
                  codeInline<Message>(route.assignmentRef),
                ],
              ),
            ],
          ),

          h.section([Ui.className<Message>(panel)], [
            h.div([Ui.className<Message>('grid gap-4')], [
              h.p([Ui.className<Message>(eyebrow)], ['Closeout path']),
              h.ol(
                [Ui.className<Message>('grid gap-3 p-0 sm:grid-cols-4')],
                statusStages.map(([label, copy]) => stage<Message>(label, copy)),
              ),
            ]),
          ]),

          h.section([Ui.className<Message>(panel)], [
            h.div([Ui.className<Message>('grid gap-4')], [
              h.p([Ui.className<Message>(eyebrow)], ['Owner-scoped commands']),
              h.p([Ui.className<Message>(bodyClass)], [
                'The browser page does not ask for or store an agent token. Run these locally from the owning Pylon environment for live data.',
              ]),
              commandBlock<Message>(statusCommand),
              commandBlock<Message>(proofCommand),
            ]),
          ]),

          h.section([Ui.className<Message>(panel)], [
            h.div([Ui.className<Message>('grid gap-3')], [
              h.p([Ui.className<Message>(eyebrow)], ['Green evidence']),
              h.p([Ui.className<Message>(bodyClass)], [
                'The promise can go green for this assignment only when status shows closeout-ready or closed-out lifecycle evidence and proof returns an empty ',
                codeInline<Message>('proofChecklist.blockerRefs'),
                ' array with exact ',
                codeInline<Message>('pylon-codex-own-capacity'),
                ' token rows.',
              ]),
            ]),
          ]),
        ],
      ),
    ],
  )
}

export const title = (route: PylonCodexAssignmentStatusRouteLike): string =>
  `Pylon Codex assignment ${route.assignmentRef} - OpenAgents`
