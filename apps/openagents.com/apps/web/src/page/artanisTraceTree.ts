import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

const pageShellClass = 'min-h-screen bg-[#08090a] text-[#f4f1e8]'

const shell =
  'mx-auto grid min-h-screen w-full max-w-7xl gap-6 px-4 py-6 sm:px-6 lg:px-8'

const panel = 'border border-white/10 bg-[#0d0f13] p-4 sm:p-5'

const eyebrow =
  'm-0 text-[0.68rem] font-semibold uppercase leading-none tracking-[0.14em] text-[#ffb400]/75'

const titleClass =
  'm-0 max-w-4xl text-3xl font-semibold leading-tight text-white sm:text-4xl'

const bodyClass = 'm-0 max-w-4xl text-sm leading-6 text-white/62'

const codeClass =
  'break-all rounded bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-white/82'

type TreeNode = Readonly<{
  label: string
  ref: string
  state: 'ready' | 'running' | 'blocked'
  detail: string
  children: ReadonlyArray<TreeNode>
}>

const tree: TreeNode = {
  label: 'FrlmConductor',
  ref: 'program_signature.frlm_conductor.v1',
  state: 'ready',
  detail: 'Environment, scheduler, budget policy, trace emitter',
  children: [
    {
      label: 'Run.Init',
      ref: 'trace.rlm.run_init',
      state: 'ready',
      detail: 'Context fragments and root task refs are bounded before fanout',
      children: [],
    },
    {
      label: 'SubQuery.Submit',
      ref: 'program_signature.rlm_leaf_executor.v1',
      state: 'running',
      detail: 'Typed leaf calls may target local, swarm, remote, or Codex lanes',
      children: [
        {
          label: 'Local',
          ref: 'executor.local.ref_only',
          state: 'ready',
          detail: 'Same-device deterministic work and public-safe adapters',
          children: [],
        },
        {
          label: 'Swarm',
          ref: 'executor.nip90.ref_only',
          state: 'running',
          detail: 'Federated work is quorum-scored before composition',
          children: [],
        },
        {
          label: 'Codex',
          ref: 'executor.pylon_codex.ref_only',
          state: 'running',
          detail: 'Owner-local coding turns stay private; token rows are exact',
          children: [],
        },
      ],
    },
    {
      label: 'SubQuery.Return',
      ref: 'evidence.rlm_trace.redacted_operator_projection',
      state: 'ready',
      detail: 'Only refs, counts, states, and gate evidence reach this page',
      children: [],
    },
    {
      label: 'Run.Done',
      ref: 'release_gate.rlm_trace.redacted_operator_projection',
      state: 'blocked',
      detail: 'Final composition stays blocked until every evidence ref exists',
      children: [],
    },
  ],
}

const signatureRefs = [
  'program_signature.frlm_conductor.v1',
  'program_signature.rlm_leaf_executor.v1',
  'program_signature.blueprint_action_submission.evidence_only.v1',
  'autonomous-ops-v1.signature-4.command-execution-source-verified',
]

const authorityRows = [
  ['Execution', 'No direct execution authority'],
  ['Payment', 'No payout or settlement authority'],
  ['Claims', 'No public-promise promotion authority'],
  ['Training', 'No checkpoint or training-promotion authority'],
] as const

const codeInline = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.code([Ui.className<Message>(codeClass)], [text])
}

const stateClass = (state: TreeNode['state']): string =>
  state === 'ready'
    ? 'border-emerald-300/25 bg-emerald-300/10 text-emerald-100'
    : state === 'running'
      ? 'border-sky-300/25 bg-sky-300/10 text-sky-100'
      : 'border-[#ffb400]/25 bg-[#ffb400]/10 text-[#ffe0a3]'

const stateLabel = (state: TreeNode['state']): string =>
  state === 'ready' ? 'ready' : state === 'running' ? 'running' : 'blocked'

const treeNode = <Message>(node: TreeNode, depth = 0): Html => {
  const h = html<Message>()
  const hasChildren = node.children.length > 0

  return h.li(
    [Ui.className<Message>('grid gap-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            `grid gap-2 border border-white/10 bg-black/25 p-3 ${depth === 0 ? '' : 'ml-4 sm:ml-6'}`,
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'text-sm font-semibold leading-5 text-white',
                  ),
                ],
                [node.label],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    `rounded border px-2 py-0.5 text-[0.68rem] font-semibold uppercase leading-none tracking-[0.12em] ${stateClass(node.state)}`,
                  ),
                ],
                [stateLabel(node.state)],
              ),
            ],
          ),
          codeInline<Message>(node.ref),
          h.p([Ui.className<Message>('m-0 text-sm leading-5 text-white/58')], [
            node.detail,
          ]),
        ],
      ),
      hasChildren
        ? h.ol(
            [
              Ui.className<Message>(
                'grid gap-3 border-l border-white/10 pl-3 sm:pl-4',
              ),
            ],
            node.children.map(child => treeNode<Message>(child, depth + 1)),
          )
        : h.span([Ui.className<Message>('sr-only')], ['Leaf node']),
    ],
  )
}

const signatureList = <Message>(): Html => {
  const h = html<Message>()

  return h.ul(
    [Ui.className<Message>('grid gap-2 p-0')],
    signatureRefs.map(ref =>
      h.li(
        [
          Ui.className<Message>(
            'border border-white/10 bg-white/[0.035] p-3 text-sm leading-5 text-white/70',
          ),
        ],
        [codeInline<Message>(ref)],
      ),
    ),
  )
}

const authorityTable = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid overflow-hidden border border-white/10 text-sm leading-5',
      ),
    ],
    authorityRows.map(([label, value]) =>
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 border-b border-white/10 p-3 last:border-b-0 sm:grid-cols-[8rem_1fr]',
          ),
        ],
        [
          h.span([Ui.className<Message>('font-semibold text-white/82')], [
            label,
          ]),
          h.span([Ui.className<Message>('text-white/58')], [value]),
        ],
      ),
    ),
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(pageShellClass),
      h.DataAttribute('route', 'artanis-traces'),
    ],
    [
      PublicHeader.view(authState),
      h.main(
        [Ui.className<Message>(shell), h.AriaLabel('Artanis RLM traces')],
        [
          h.section(
            [
              Ui.className<Message>(
                'grid gap-5 border border-white/10 bg-[#0d0f13] p-5 sm:p-6',
              ),
            ],
            [
              h.p([Ui.className<Message>(eyebrow)], ['RLM trace visualizer']),
              h.h1([Ui.className<Message>(titleClass)], [
                'Artanis execution tree',
              ]),
              h.p([Ui.className<Message>(bodyClass)], [
                'A ref-only view of the Recursive Language Model shape behind Artanis: conductor, fanout, typed leaf executors, returned evidence, and composition gates.',
              ]),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-3 border border-white/10 bg-black/25 p-3 text-sm leading-5 text-white/60 sm:grid-cols-3',
                  ),
                ],
                [
                  h.div([], [
                    h.span([Ui.className<Message>('block text-white/42')], [
                      'Source',
                    ]),
                    codeInline<Message>('/api/operator/rlm/traces'),
                  ]),
                  h.div([], [
                    h.span([Ui.className<Message>('block text-white/42')], [
                      'Projection',
                    ]),
                    codeInline<Message>('openagents.operator.rlm_traces.v1'),
                  ]),
                  h.div([], [
                    h.span([Ui.className<Message>('block text-white/42')], [
                      'Privacy',
                    ]),
                    codeInline<Message>('operator_refs_only'),
                  ]),
                ],
              ),
            ],
          ),
          h.section(
            [Ui.className<Message>('grid gap-6 lg:grid-cols-[1.45fr_0.9fr]')],
            [
              h.div([Ui.className<Message>(panel)], [
                h.div([Ui.className<Message>('grid gap-4')], [
                  h.p([Ui.className<Message>(eyebrow)], ['Execution tree']),
                  h.ol([Ui.className<Message>('grid gap-3 p-0')], [
                    treeNode<Message>(tree),
                  ]),
                ]),
              ]),
              h.aside([Ui.className<Message>('grid content-start gap-6')], [
                h.section([Ui.className<Message>(panel)], [
                  h.div([Ui.className<Message>('grid gap-4')], [
                    h.p([Ui.className<Message>(eyebrow)], [
                      'Blueprint signatures',
                    ]),
                    signatureList<Message>(),
                  ]),
                ]),
                h.section([Ui.className<Message>(panel)], [
                  h.div([Ui.className<Message>('grid gap-4')], [
                    h.p([Ui.className<Message>(eyebrow)], [
                      'Authority boundary',
                    ]),
                    authorityTable<Message>(),
                  ]),
                ]),
              ]),
            ],
          ),
        ],
      ),
    ],
  )
}

export const title = (): string => 'Artanis RLM traces - OpenAgents'
