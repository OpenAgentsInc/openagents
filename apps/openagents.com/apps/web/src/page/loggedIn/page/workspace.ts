import { Array, Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { autopilotWorkRouter } from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  Model,
  PrefilledWorkspace,
  PrefilledWorkspaceSeededMemoryEntry,
  PrefilledWorkspaceStarterWorkflow,
} from '../model'

const statusLabel = (status: string): string => status.replaceAll('_', ' ')

const statusToneClass = (status: string): string =>
  M.value(status).pipe(
    M.when('active', () => 'border-[#00c853]/40 text-[#00c853]'),
    M.when('ready', () => 'border-[#00c853]/40 text-[#00c853]'),
    M.when('completed', () => 'border-[#00c853]/40 text-[#00c853]'),
    M.when('invited', () => 'border-[#ffb400]/45 text-[#ffb400]'),
    M.when('queued', () => 'border-[#ffb400]/45 text-[#ffb400]'),
    M.when('draft', () => 'border-white/20 text-white/55'),
    M.when('dismissed', () => 'border-white/20 text-white/45'),
    M.when('archived', () => 'border-white/15 text-white/35'),
    M.orElse(() => 'border-white/20 text-white/55'),
  )

const badge = (status: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex w-fit items-center border px-2 py-1 font-mono text-[0.6875rem] uppercase leading-none ${statusToneClass(status)}`,
      ),
    ],
    [statusLabel(status)],
  )
}

const sourceRef = (ref: string): Html => {
  const h = html<Message>()
  const trimmed = ref.trim()
  const isUrl = /^https?:\/\//.test(trimmed)

  return isUrl
    ? h.a(
        [
          h.Href(trimmed),
          h.Target('_blank'),
          h.Rel('noreferrer'),
          Ui.className<Message>(
            'break-all text-[#ffb400] underline-offset-2 hover:underline',
          ),
        ],
        [trimmed],
      )
    : h.span([Ui.className<Message>('break-all text-white/45')], [trimmed])
}

const emptyPanel = (title: string, body: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border border-[#222] bg-[#010102] p-4')],
    [
      h.h3(
        [Ui.className<Message>('m-0 text-base font-medium text-[#f1efe8]')],
        [title],
      ),
      h.p([Ui.className<Message>('mt-2 text-base/7 text-white/55')], [body]),
    ],
  )
}

const memoryRow = (entry: PrefilledWorkspaceSeededMemoryEntry): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-3 border-t border-[#181818] py-3 first:border-t-0 md:grid-cols-[minmax(8rem,13rem)_minmax(0,1fr)]',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'font-mono text-[0.75rem] uppercase leading-5 text-white/45',
          ),
        ],
        [entry.label],
      ),
      h.div(
        [Ui.className<Message>('grid min-w-0 gap-2')],
        [
          h.p(
            [Ui.className<Message>('m-0 text-base/7 text-white/75')],
            [entry.value],
          ),
          h.div(
            [Ui.className<Message>('font-mono text-xs')],
            [sourceRef(entry.publicSourceRef)],
          ),
        ],
      ),
    ],
  )
}

const seededMemoryView = (
  entries: ReadonlyArray<PrefilledWorkspaceSeededMemoryEntry>,
): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Memory',
        title: 'Seeded notes',
        body: 'Public-source facts prepared for this project.',
        level: 2,
      }),
      Array.isReadonlyArrayEmpty(entries)
        ? emptyPanel(
            'No seeded notes yet',
            'This workspace has no seeded public-source notes yet.',
          )
        : h.ul(
            [
              h.Role('list'),
              Ui.className<Message>(
                'mt-4 border border-[#222] bg-[#010102] px-4',
              ),
            ],
            entries.map(memoryRow),
          ),
    ],
    [Ui.className<Message>('mt-5')],
  )
}

const workflowRow = (
  workflow: PrefilledWorkspaceStarterWorkflow,
  index: number,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-3 border-t border-[#181818] py-4 first:border-t-0 lg:grid-cols-[minmax(0,1fr)_9rem]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('grid min-w-0 gap-2')],
        [
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              h.h3(
                [
                  Ui.className<Message>(
                    'm-0 text-base font-medium text-[#f1efe8]',
                  ),
                ],
                [`${index + 1}. ${workflow.title}`],
              ),
              badge(workflow.status),
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-base/7 text-white/60')],
            [workflow.description],
          ),
          h.div(
            [
              Ui.className<Message>(
                'font-mono text-xs uppercase text-white/35',
              ),
            ],
            [workflow.outcomeKind],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex items-start lg:justify-end')],
        [
          h.a(
            [
              h.Href(autopilotWorkRouter()),
              Ui.className<Message>(
                'inline-flex min-h-9 items-center border border-[#f1efe8] bg-[#f1efe8] px-3 font-mono text-[0.8125rem] text-black hover:bg-white',
              ),
            ],
            ['Open Work'],
          ),
        ],
      ),
    ],
  )
}

const starterWorkflowsView = (
  workflows: ReadonlyArray<PrefilledWorkspaceStarterWorkflow>,
): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Starters',
        title: 'Starter workflows',
        body: 'Accepted-outcome workflows prepared for the first handoff.',
        level: 2,
      }),
      Array.isReadonlyArrayEmpty(workflows)
        ? emptyPanel(
            'No starter workflows yet',
            'This workspace has no starter workflows yet.',
          )
        : h.ul(
            [
              h.Role('list'),
              Ui.className<Message>(
                'mt-4 border border-[#222] bg-[#010102] px-4',
              ),
            ],
            workflows.map(workflowRow),
          ),
    ],
    [Ui.className<Message>('mt-5')],
  )
}

const introReceiptView = (workspace: PrefilledWorkspace): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      h.div(
        [
          Ui.className<Message>(
            'grid gap-4 border border-[#222] bg-[#010102] p-4 md:grid-cols-[minmax(0,1fr)_minmax(12rem,18rem)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
                  ),
                ],
                ['Intro receipt'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-base/7 text-white/70')],
                [workspace.introReceipt.summary],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid content-start gap-2')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
                  ),
                ],
                ['Sources'],
              ),
              Array.isReadonlyArrayEmpty(
                workspace.introReceipt.publicSourceRefs,
              )
                ? h.p(
                    [Ui.className<Message>('m-0 text-sm text-white/40')],
                    ['No public sources attached.'],
                  )
                : h.ul(
                    [h.Role('list'), Ui.className<Message>('grid gap-2')],
                    workspace.introReceipt.publicSourceRefs.map(ref =>
                      h.li(
                        [Ui.className<Message>('font-mono text-xs')],
                        [sourceRef(ref)],
                      ),
                    ),
                  ),
            ],
          ),
        ],
      ),
    ],
    [Ui.className<Message>('mt-5')],
  )
}

const loadedView = (input: {
  readonly generatedAt: string
  readonly workspace: PrefilledWorkspace
}): Html => {
  const h = html<Message>()
  const { workspace } = input

  return h.div(
    [
      h.DataAttribute('route', 'prefilled-workspace'),
      Ui.className<Message>('mx-auto w-[min(100%,960px)] px-4 py-6'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-4',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid min-w-0 gap-2')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
                  ),
                ],
                ['Project workspace'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-2xl font-medium tracking-normal text-[#f1efe8] sm:text-3xl',
                  ),
                ],
                [workspace.projectName],
              ),
              h.p(
                [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
                [`${workspace.id} · generated ${input.generatedAt}`],
              ),
            ],
          ),
          badge(workspace.status),
        ],
      ),
      introReceiptView(workspace),
      starterWorkflowsView(workspace.starterWorkflows),
      seededMemoryView(workspace.seededMemory),
    ],
  )
}

const loadingView = (workspaceId: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'prefilled-workspace'),
      Ui.className<Message>(
        'mx-auto grid w-[min(100%,720px)] gap-3 px-4 py-10',
      ),
    ],
    [
      h.p(
        [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
        [workspaceId],
      ),
      h.h1(
        [Ui.className<Message>('m-0 text-2xl font-medium text-[#f1efe8]')],
        ['Loading workspace'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        ['Opening the project setup.'],
      ),
    ],
  )
}

const failedView = (workspaceId: string, error: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'prefilled-workspace'),
      Ui.className<Message>(
        'mx-auto grid w-[min(100%,720px)] gap-4 px-4 py-10',
      ),
    ],
    [
      h.p(
        [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
        [workspaceId],
      ),
      h.h1(
        [Ui.className<Message>('m-0 text-2xl font-medium text-[#f1efe8]')],
        ['Workspace unavailable'],
      ),
      h.p([Ui.className<Message>('m-0 text-base/7 text-white/60')], [error]),
      h.a(
        [
          h.Href(autopilotWorkRouter()),
          Ui.className<Message>(
            'inline-flex min-h-9 w-fit items-center border border-[#f1efe8] bg-[#f1efe8] px-3 font-mono text-[0.8125rem] text-black hover:bg-white',
          ),
        ],
        ['Open Work'],
      ),
    ],
  )
}

export const view = (model: Model): Html =>
  M.value(model.prefilledWorkspace).pipe(
    M.tagsExhaustive({
      PrefilledWorkspaceIdle: () =>
        model.route._tag === 'Workspace'
          ? loadingView(model.route.workspaceId)
          : loadingView('workspace'),
      PrefilledWorkspaceLoading: ({ workspaceId }) => loadingView(workspaceId),
      PrefilledWorkspaceLoaded: ({ generatedAt, workspace }) =>
        loadedView({ generatedAt, workspace }),
      PrefilledWorkspaceFailed: ({ error, workspaceId }) =>
        failedView(workspaceId, error),
    }),
  )
