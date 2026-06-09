import { Match as M, Option } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  ClickedArtanisOperatorApprovalAction,
  ClickedArtanisOperatorGoalAction,
  SubmittedArtanisOperatorGoal,
  UpdatedArtanisOperatorGoalObjectiveDraft,
  type Message,
} from '../message'
import type {
  ArtanisOperatorConsoleApprovalGate,
  ArtanisOperatorConsoleResponse,
  ArtanisOperatorConsoleWorkProposal,
  Model,
} from '../model'

const humanLabel = (value: string): string => value.replace(/_/g, ' ')

const refsView = (
  refs: ReadonlyArray<string>,
  emptyLabel = 'none',
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 text-[0.6875rem] leading-4 text-white/45')],
    refs.length === 0
      ? [h.span([], [emptyLabel])]
      : refs.slice(0, 3).map(ref =>
          h.span(
            [Ui.className<Message>('break-words [overflow-wrap:anywhere]')],
            [ref],
          ),
        ),
  )
}

const valueText = (value: string | number | null): Html =>
  html<Message>().span(
    [Ui.className<Message>('text-[0.75rem] leading-5 text-white/60')],
    [value === null ? 'none' : String(value)],
  )

const statusRows = (
  response: ArtanisOperatorConsoleResponse,
): ReadonlyArray<Html> => [
  Ui.workroomPanelActionRow<Message>({
    label: 'Runtime',
    action: valueText(humanLabel(response.status.runtimeState)),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Loop',
    action: valueText(humanLabel(response.status.loopState)),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Health',
    action: valueText(humanLabel(response.status.healthState)),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Last tick',
    action: valueText(response.status.lastTickRef),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Next tick',
    action: valueText(response.status.nextTickDisplay),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Approvals',
    action: valueText(response.status.pendingApprovalCount),
  }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Forum lag',
    action: valueText(humanLabel(response.status.publicationLagState)),
  }),
]

const goalControls = (model: Model): Html => {
  const h = html<Message>()
  const panel = model.artanisOperatorGoalPanel
  const busy = Option.isSome(panel.pendingAction)
  const goal = Option.getOrNull(panel.goal)
  const buttonLabel = busy
    ? 'Saving'
    : goal === null
      ? 'Create'
      : 'Reprioritize'

  return h.form(
    [h.OnSubmit(SubmittedArtanisOperatorGoal()), Ui.className<Message>('grid gap-2')],
    [
      h.textarea(
        [
          h.Name('artanisOperatorGoal'),
          h.AriaLabel('Artanis operator goal'),
          h.Rows(3),
          h.Value(panel.objectiveDraft),
          h.OnInput(value =>
            UpdatedArtanisOperatorGoalObjectiveDraft({ value }),
          ),
          Ui.className<Message>(
            `${Ui.inputClass} min-h-20 resize-y text-[0.8125rem] leading-5 max-sm:text-base`,
          ),
        ],
        [],
      ),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        Ui.compactButton<Message>({
          label: buttonLabel,
          variant: 'strong',
          attrs: [h.Type('submit'), ...(busy ? [h.Disabled(true)] : [])],
        }),
        goal === null
          ? null
          : goal.status === 'active'
            ? Ui.compactButton<Message>({
                label: 'Pause',
                attrs: [
                  h.Type('button'),
                  ...(busy ? [h.Disabled(true)] : []),
                  h.OnClick(
                    ClickedArtanisOperatorGoalAction({ action: 'pause' }),
                  ),
                ],
              })
            : Ui.compactButton<Message>({
                label: 'Resume',
                attrs: [
                  h.Type('button'),
                  ...(busy ? [h.Disabled(true)] : []),
                  h.OnClick(
                    ClickedArtanisOperatorGoalAction({ action: 'resume' }),
                  ),
                ],
              }),
        goal === null
          ? null
          : Ui.compactButton<Message>({
              label: 'Cancel',
              attrs: [
                h.Type('button'),
                ...(busy ? [h.Disabled(true)] : []),
                h.OnClick(
                  ClickedArtanisOperatorGoalAction({ action: 'clear' }),
                ),
              ],
            }),
      ]),
      Option.match(panel.error, {
        onNone: () => null,
        onSome: error =>
          h.p([Ui.className<Message>('m-0 text-[0.75rem] text-[#ff6f00]')], [
            error,
          ]),
      }),
    ],
  )
}

const approvalGateView = (
  gate: ArtanisOperatorConsoleApprovalGate,
): Html => {
  const h = html<Message>()

  return h.li(
    [Ui.className<Message>('grid gap-2 border-t border-white/10 pt-2')],
    [
      h.div(
        [Ui.className<Message>('flex items-start justify-between gap-3')],
        [
          h.span(
            [Ui.className<Message>('text-[0.75rem] leading-5 text-white/75')],
            [humanLabel(gate.kind)],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] leading-5 text-white/45')],
            [humanLabel(gate.state)],
          ),
        ],
      ),
      refsView([
        ...gate.operatorReceiptRefs,
        ...gate.authorityReceiptRefs,
        ...gate.privateEvidenceRefs,
      ]),
      gate.state === 'pending'
        ? h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
            Ui.compactButton<Message>({
              label: 'Approve',
              attrs: [
                h.Type('button'),
                h.OnClick(
                  ClickedArtanisOperatorApprovalAction({
                    action: 'approve',
                    gateRef: gate.gateRef,
                  }),
                ),
              ],
            }),
            Ui.compactButton<Message>({
              label: 'Reject',
              attrs: [
                h.Type('button'),
                h.OnClick(
                  ClickedArtanisOperatorApprovalAction({
                    action: 'reject',
                    gateRef: gate.gateRef,
                  }),
                ),
              ],
            }),
          ])
        : valueText(gate.label),
    ],
  )
}

const workProposalView = (
  proposal: ArtanisOperatorConsoleWorkProposal,
): Html => {
  const h = html<Message>()

  return h.li(
    [Ui.className<Message>('grid gap-1 border-t border-white/10 pt-2')],
    [
      h.div([Ui.className<Message>('text-[0.75rem] leading-5 text-white/70')], [
        `${humanLabel(proposal.target)} / ${humanLabel(proposal.capability)}`,
      ]),
      h.div([Ui.className<Message>('text-[0.6875rem] leading-4 text-white/45')], [
        `${humanLabel(proposal.state)} · ${humanLabel(proposal.risk)}`,
      ]),
      refsView([
        ...proposal.approvalRequirementRefs,
        ...proposal.operatorDetailRefs,
        ...proposal.spendLimitRefs,
      ]),
    ],
  )
}

const loadedConsoleView = (
  model: Model,
  response: ArtanisOperatorConsoleResponse,
): Html => {
  const h = html<Message>()
  const gates = response.approvalGates?.gates ?? []
  const proposals = response.workRouting?.proposals ?? []
  const intents = response.publicationQueue?.intents ?? []

  return h.div([Ui.className<Message>('grid gap-3')], [
    ...statusRows(response),
    h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Lifecycle']),
    goalControls(model),
    h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Private refs']),
    refsView([
      ...response.steering.privateEvidencePackRefs,
      ...response.steering.rawWorkroomStateRefs,
    ]),
    h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Approval gates']),
    gates.length === 0
      ? valueText('none pending')
      : h.ol([Ui.className<Message>('grid gap-2')], gates.slice(0, 3).map(approvalGateView)),
    h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Work routing']),
    proposals.length === 0
      ? valueText('none proposed')
      : h.ol([Ui.className<Message>('grid gap-2')], proposals.slice(0, 3).map(workProposalView)),
    h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Publication queue']),
    valueText(
      response.publicationQueue === null
        ? 'no intents'
        : `${response.publicationQueue.deliverableIntentRefs.length} ready / ${intents.length} total`,
    ),
  ])
}

const consoleBody = (model: Model): Html =>
  M.value(model.artanisOperatorConsole).pipe(
    M.tagsExhaustive({
      ArtanisOperatorConsoleIdle: () => valueText('loading'),
      ArtanisOperatorConsoleLoading: () => valueText('loading'),
      ArtanisOperatorConsoleFailed: ({ error }) => valueText(error),
      ArtanisOperatorConsoleLoaded: ({ response }) =>
        loadedConsoleView(model, response),
    }),
  )

export const artanisOperatorDock = (model: Model): Html | null => {
  const h = html<Message>()

  if (!model.auth.isAdmin || model.route._tag !== 'Chat') {
    return null
  }

  return h.section(
    [
      h.DataAttribute('component', 'artanis-operator-console'),
      Ui.className<Message>('grid gap-3'),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [
        'Artanis operator',
      ]),
      consoleBody(model),
    ],
  )
}
