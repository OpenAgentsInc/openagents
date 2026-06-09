import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { Message } from '../message'
import {
  AgentGoalPanelModel,
  Model,
} from '../model'
import {
  noUpdate,
  type UpdateReturn,
} from '../transition'
import {
  SaveAgentGoal,
  UpdateAgentGoalAction,
} from './commands'
import { agentGoalScopeForRoute } from './scope'
import type { AgentGoalScopeRequest } from './scope'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const formatBudgetDraft = (budget: number | null): string =>
  budget === null ? '' : String(budget)

export const modelWithAgentGoal = (
  model: Model,
  goal: Model['agentGoalPanel']['goal'],
  scopeKey: string,
): Model =>
  evo(model, {
    agentGoalPanel: panel =>
      AgentGoalPanelModel({
        ...panel,
        budgetDraft: Option.match(goal, {
          onNone: () => '',
          onSome: current => formatBudgetDraft(current.tokenBudget),
        }),
        error: Option.none(),
        goal,
        isEditing: Option.isNone(goal),
        objectiveDraft: Option.match(goal, {
          onNone: () => panel.objectiveDraft,
          onSome: current => current.objective,
        }),
        pendingAction: Option.none(),
        scopeKey,
      }),
  })

const parseBudgetDraft = (
  value: string,
): { readonly _tag: 'Valid'; readonly value: number | null } | {
  readonly _tag: 'Invalid'
  readonly error: string
} => {
  const trimmed = value.trim()

  if (trimmed === '') {
    return { _tag: 'Valid', value: null }
  }

  const parsed = Number(trimmed)

  return Number.isInteger(parsed) && parsed > 0
    ? { _tag: 'Valid', value: parsed }
    : { _tag: 'Invalid', error: 'Budget must be a positive whole number.' }
}

const saveCommandInput = (
  scope: AgentGoalScopeRequest,
  input: Readonly<{
    goalId: string | undefined
    objective: string
    tokenBudget: number | null
  }>,
): Parameters<typeof SaveAgentGoal>[0] => ({
  agentId: scope.agentId,
  objective: input.objective,
  scopeKey: scope.scopeKey,
  tokenBudget: input.tokenBudget,
  ...(input.goalId === undefined ? {} : { goalId: input.goalId }),
  ...(scope.projectId === undefined ? {} : { projectId: scope.projectId }),
  ...(scope.teamId === undefined ? {} : { teamId: scope.teamId }),
})

export const updateAgentGoals = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadAgentGoal: ({ scopeKey }) => [
        evo(model, {
          agentGoalPanel: panel =>
            AgentGoalPanelModel({
              ...panel,
              error: Option.none(),
              pendingAction: Option.some('Loading'),
              scopeKey,
            }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadAgentGoal: ({ response, scopeKey }) =>
        [
          modelWithAgentGoal(
            model,
            response.goal === null ? Option.none() : Option.some(response.goal),
            scopeKey,
          ),
          [],
          Option.none(),
        ],
      FailedLoadAgentGoal: ({ error, scopeKey }) =>
        scopeKey !== model.agentGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                agentGoalPanel: panel =>
                  AgentGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
      UpdatedAgentGoalObjectiveDraft: ({ value }) => [
        evo(model, {
          agentGoalPanel: panel =>
            AgentGoalPanelModel({ ...panel, objectiveDraft: value }),
        }),
        [],
        Option.none(),
      ],
      UpdatedAgentGoalBudgetDraft: ({ value }) => [
        evo(model, {
          agentGoalPanel: panel =>
            AgentGoalPanelModel({ ...panel, budgetDraft: value }),
        }),
        [],
        Option.none(),
      ],
      ClickedEditAgentGoal: () => [
        evo(model, {
          agentGoalPanel: panel =>
            AgentGoalPanelModel({
              ...panel,
              isEditing: true,
            }),
        }),
        [],
        Option.none(),
      ],
      ClickedCancelEditAgentGoal: () => [
        modelWithAgentGoal(model, model.agentGoalPanel.goal, model.agentGoalPanel.scopeKey),
        [],
        Option.none(),
      ],
      SubmittedAgentGoal: () => {
        const scope = agentGoalScopeForRoute(model)

        if (scope === undefined) {
          return noUpdate(model)
        }

        const objective = model.agentGoalPanel.objectiveDraft.trim()

        if (objective === '') {
          return [
            evo(model, {
              agentGoalPanel: panel =>
                AgentGoalPanelModel({
                  ...panel,
                  error: Option.some('Goal is required.'),
                }),
            }),
            [],
            Option.none(),
          ]
        }

        const budget = parseBudgetDraft(model.agentGoalPanel.budgetDraft)

        if (budget._tag === 'Invalid') {
          return [
            evo(model, {
              agentGoalPanel: panel =>
                AgentGoalPanelModel({
                  ...panel,
                  error: Option.some(budget.error),
                }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            agentGoalPanel: panel =>
              AgentGoalPanelModel({
                ...panel,
                error: Option.none(),
                pendingAction: Option.some('Saving'),
                scopeKey: scope.scopeKey,
              }),
          }),
          [
            SaveAgentGoal(saveCommandInput(scope, {
              goalId: Option.getOrUndefined(
                Option.map(model.agentGoalPanel.goal, goal => goal.id),
              ),
              objective,
              tokenBudget: budget.value,
            })),
          ],
          Option.none(),
        ]
      },
      SucceededSaveAgentGoal: ({ response, scopeKey }) =>
        [
          modelWithAgentGoal(
            model,
            response.goal === null ? Option.none() : Option.some(response.goal),
            scopeKey,
          ),
          [],
          Option.none(),
        ],
      FailedSaveAgentGoal: ({ error, scopeKey }) =>
        scopeKey !== model.agentGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                agentGoalPanel: panel =>
                  AgentGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
      ClickedAgentGoalAction: ({ action }) =>
        Option.match(model.agentGoalPanel.goal, {
          onNone: () => noUpdate(model),
          onSome: goal => [
            evo(model, {
              agentGoalPanel: panel =>
                AgentGoalPanelModel({
                  ...panel,
                  error: Option.none(),
                  pendingAction: Option.some(action),
                }),
            }),
            [
              UpdateAgentGoalAction({
                action,
                goalId: goal.id,
                scopeKey: model.agentGoalPanel.scopeKey,
              }),
            ],
            Option.none(),
          ],
        }),
      SucceededAgentGoalAction: ({ action, response, scopeKey }) =>
        [
          modelWithAgentGoal(
            model,
            action === 'clear' || response.goal === null
              ? Option.none()
              : Option.some(response.goal),
            scopeKey,
          ),
          [],
          Option.none(),
        ],
      FailedAgentGoalAction: ({ error, scopeKey }) =>
        scopeKey !== model.agentGoalPanel.scopeKey
          ? noUpdate(model)
          : [
              evo(model, {
                agentGoalPanel: panel =>
                  AgentGoalPanelModel({
                    ...panel,
                    error: Option.some(error),
                    pendingAction: Option.none(),
                  }),
              }),
              [],
              Option.none(),
            ],
    }),
    M.orElse(() => noUpdate(model)),
  )
