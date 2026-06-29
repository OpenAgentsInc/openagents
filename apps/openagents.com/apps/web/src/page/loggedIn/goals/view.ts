import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  ClickedAgentGoalAction,
  ClickedCancelEditAgentGoal,
  ClickedEditAgentGoal,
  SubmittedAgentGoal,
  UpdatedAgentGoalBudgetDraft,
  UpdatedAgentGoalObjectiveDraft,
  type Message,
} from '../message'
import type {
  AgentGoalAction,
  AgentGoalApiGoal,
  Model,
} from '../model'

const statusLabel = (goal: AgentGoalApiGoal): string =>
  goal.status.replace(/_/g, ' ')

const usageLabel = (goal: AgentGoalApiGoal): string => {
  const used = goal.tokensUsed === 0 ? 'pending' : String(goal.tokensUsed)

  return goal.remainingTokens === null
    ? used
    : `${used} / ${goal.tokenBudget ?? 0}`
}

const actionButton = (
  label: string,
  action: AgentGoalAction,
  disabled: boolean,
): Html => {
  const h = html<Message>()

  return Ui.compactButton<Message>({
    label,
    attrs: [
      h.AriaLabel(label),
      ...(disabled ? [h.Disabled(true)] : []),
      h.OnClick(ClickedAgentGoalAction({ action })),
    ],
  })
}

const goalRows = (goal: AgentGoalApiGoal): ReadonlyArray<Html> => {
  const h = html<Message>()

  return [
    Ui.workroomPanelActionRow<Message>({
      label: 'Status',
      action: h.span([Ui.className<Message>('text-[0.75rem] text-white/55')], [
        statusLabel(goal),
      ]),
    }),
    Ui.workroomPanelActionRow<Message>({
      label: 'Tokens',
      action: h.span([Ui.className<Message>('text-[0.75rem] text-white/55')], [
        usageLabel(goal),
      ]),
    }),
  ]
}

const goalActions = (
  goal: AgentGoalApiGoal,
  busy: boolean,
): ReadonlyArray<Html> => [
  Ui.workroomPanelActionRow<Message>({
    label: 'Control',
    action:
      goal.canPause && goal.status === 'active'
        ? actionButton('Pause', 'pause', busy)
        : actionButton('Resume', 'resume', busy || !goal.canResume),
  }),
  goal.canMakePublic
    ? Ui.workroomPanelActionRow<Message>({
        label: 'Visibility',
        action: actionButton('Public', 'make_public', busy),
      })
    : Ui.workroomPanelActionRow<Message>({
        label: 'Visibility',
        action: html<Message>().span(
          [Ui.className<Message>('text-[0.75rem] text-white/55')],
          [goal.visibility],
        ),
      }),
  Ui.workroomPanelActionRow<Message>({
    label: 'Goal',
    action: Ui.compactButton<Message>({
      label: 'Clear',
      attrs: [
        html<Message>().AriaLabel('Clear goal'),
        ...(busy ? [html<Message>().Disabled(true)] : []),
        html<Message>().OnClick(ClickedAgentGoalAction({ action: 'clear' })),
      ],
    }),
  }),
]

const editForm = (model: Model, buttonLabel: string): Html => {
  const h = html<Message>()
  const busy = Option.isSome(model.agentGoalPanel.pendingAction)

  return h.form(
    [
      h.OnSubmit(SubmittedAgentGoal()),
      Ui.className<Message>('grid gap-2.5'),
    ],
    [
      h.textarea(
        [
          h.Name('agentGoalObjective'),
          h.Rows(4),
          h.Placeholder('Ship the next production outcome'),
          h.OnInput(value => UpdatedAgentGoalObjectiveDraft({ value })),
          Ui.className<Message>(
            `${Ui.inputClass} min-h-24 resize-y text-[0.8125rem] leading-5 max-sm:text-base`,
          ),
        ],
        [model.agentGoalPanel.objectiveDraft],
      ),
      h.input([
        h.Name('agentGoalTokenBudget'),
        h.Type('text'),
        h.Placeholder('Token budget'),
        h.Value(model.agentGoalPanel.budgetDraft),
        h.OnInput(value => UpdatedAgentGoalBudgetDraft({ value })),
        Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
      ]),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          Ui.compactButton<Message>({
            label: busy ? 'Saving' : buttonLabel,
            variant: 'strong',
            attrs: [h.Type('submit'), ...(busy ? [h.Disabled(true)] : [])],
          }),
          model.agentGoalPanel.isEditing &&
          Option.isSome(model.agentGoalPanel.goal)
            ? Ui.compactButton<Message>({
                label: 'Cancel',
                attrs: [
                  h.AriaLabel('Cancel goal edit'),
                  h.OnClick(ClickedCancelEditAgentGoal()),
                ],
              })
            : null,
        ],
      ),
    ],
  )
}

const activeGoalPanel = (model: Model, goal: AgentGoalApiGoal): Html => {
  const h = html<Message>()
  const busy = Option.isSome(model.agentGoalPanel.pendingAction)

  return h.div(
    [Ui.className<Message>('grid gap-3')],
    [
      h.div(
        [Ui.className<Message>('grid gap-1')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Goal']),
          h.p(
            [
              Ui.className<Message>(
                'max-h-28 overflow-auto whitespace-pre-wrap text-[0.8125rem] leading-5 text-[#f1efe8]',
              ),
            ],
            [goal.objective],
          ),
        ],
      ),
      ...goalRows(goal),
      ...goalActions(goal, busy),
      Ui.compactButton<Message>({
        label: 'Edit goal',
        attrs: [
          h.AriaLabel('Edit goal'),
          ...(goal.canEdit && !busy ? [] : [h.Disabled(true)]),
          h.OnClick(ClickedEditAgentGoal()),
        ],
      }),
    ],
  )
}

export const agentGoalDock = (model: Model): Html => {
  const h = html<Message>()
  const error = Option.getOrNull(model.agentGoalPanel.error)

  return h.section(
    [
      h.DataAttribute('component', 'agent-goal-panel'),
      Ui.className<Message>('grid gap-3'),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Autopilot goal']),
      Option.match(model.agentGoalPanel.goal, {
        onNone: () => editForm(model, 'Set goal'),
        onSome: goal =>
          model.agentGoalPanel.isEditing
            ? editForm(model, 'Save goal')
            : activeGoalPanel(model, goal),
      }),
      error === null
        ? null
        : h.p([Ui.className<Message>('text-[0.75rem] text-[#ff6f00]')], [
            error,
          ]),
    ],
  )
}
