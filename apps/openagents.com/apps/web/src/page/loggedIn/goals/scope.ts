import { projectWorkroomsEnabled } from '../../../product-policy'
import {
  Model,
  syncTeamScope,
  syncWorkspaceScope,
  teamProjectRouteRef,
  teamRouteRef,
} from '../model'

const DEFAULT_AGENT_ID = 'autopilot'

export type AgentGoalScopeRequest = Readonly<{
  agentId: string
  href: string
  projectId?: string
  scopeKey: string
  teamId?: string
}>

const paramsFromScope = (
  scope: Pick<AgentGoalScopeRequest, 'agentId' | 'projectId' | 'teamId'>,
): string => {
  const params = new URLSearchParams({ agentId: scope.agentId })

  if (scope.teamId !== undefined) {
    params.set('teamId', scope.teamId)
  }

  if (scope.projectId !== undefined) {
    params.set('projectId', scope.projectId)
  }

  return params.toString()
}

const requestForScope = (
  scope: Pick<AgentGoalScopeRequest, 'agentId' | 'projectId' | 'teamId'>,
): AgentGoalScopeRequest => ({
  ...scope,
  href: `/api/autopilot/goals/current?${paramsFromScope(scope)}`,
  scopeKey: [
    scope.agentId,
    scope.teamId ?? 'personal',
    scope.projectId ?? 'room',
  ].join(':'),
})

const teamIdForRef = (model: Model, teamRef: string): string | undefined =>
  model.auth.teams.find(team => teamRouteRef(team) === teamRef)?.id

export const agentGoalScopeForRoute = (
  model: Model,
): AgentGoalScopeRequest | undefined => {
  const route = model.route

  if (route._tag === 'Chat' || route._tag === 'Thread') {
    return requestForScope({ agentId: DEFAULT_AGENT_ID })
  }

  if (route._tag === 'TeamChat') {
    const teamId = teamIdForRef(model, route.teamRef)

    return teamId === undefined
      ? undefined
      : requestForScope({ agentId: DEFAULT_AGENT_ID, teamId })
  }

  if (route._tag === 'TeamProjectChat') {
    if (!projectWorkroomsEnabled()) {
      return undefined
    }

    const team = model.auth.teams.find(
      candidate => teamRouteRef(candidate) === route.teamRef,
    )
    const project = team?.projects?.find(
      candidate => teamProjectRouteRef(candidate) === route.projectRef,
    )

    return team === undefined || project === undefined
      ? undefined
      : requestForScope({
          agentId: project.agent?.id ?? DEFAULT_AGENT_ID,
          projectId: project.id,
          teamId: team.id,
        })
  }

  return undefined
}

export const syncScopeForAgentGoalRoute = (
  model: Model,
): string | undefined => {
  const route = model.route

  if (route._tag === 'Chat' || route._tag === 'Thread') {
    return syncWorkspaceScope(model.auth.session)
  }

  if (
    route._tag !== 'TeamChat' &&
    route._tag !== 'TeamProjectChat'
  ) {
    return undefined
  }

  const teamId = teamIdForRef(model, route.teamRef)

  return teamId === undefined ? undefined : syncTeamScope(teamId)
}
