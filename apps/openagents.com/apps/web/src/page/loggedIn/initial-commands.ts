import type { Command } from 'foldkit'

import { loggedInPermissionGate } from '../../product-policy'
import { LoadAdminOverview } from './admin/transitions'
import { artanisOperatorInitialCommands } from './artanis-console/transitions'
import {
  LoadAutopilotMorningReport,
  LoadAutopilotWorkBriefing,
  LoadAutopilotWorkDetail,
  LoadAutopilotWorkEvents,
  LoadAutopilotWorkList,
  LoadCustomerOneCohort,
} from './autopilot-work/transitions'
import {
  teamChatMessagesRequestForRoute,
  threadFileDetailRequestForRoute,
  threadFilesRequestForRoute,
} from './chatState'
import {
  FocusChatComposer,
  InstallAccountMenuOutsideClick,
} from './commands/dom'
import {
  LoadCustomerOrder,
  LoadCustomerOrders,
} from './customer-order/transitions'
import { LoadAutopilotDecisions } from './decisions/transitions'
import { LoadAgentGoal } from './goals/commands'
import { agentGoalScopeForRoute } from './goals/scope'
import { Message } from './message'
import { Model } from './model'
import { LoadMulletBootstrap } from './mullet/transitions'
import { notificationInitialCommands } from './notifications/transitions'
import { LoadOnboardingRepositories } from './onboarding/transitions'
import { LoadProviderAccountPool } from './providers/commands'
import { LoadTokenUsageStats } from './stats/transitions'
import { LoadSyncSnapshot } from './sync/commands'
import { syncSnapshotHref } from './sync/projection'
import { LoadTeamChatMessages } from './team-chat/commands'
import { LoadThreadFileDetail, LoadThreadFiles } from './thread-files/commands'
import {
  LoadWorkroomLifecycle,
  LoadWorkroomSurface,
} from './workroom/transitions'
import { LoadPrefilledWorkspace } from './workspace/transitions'

export const initialCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> => {
  if (loggedInPermissionGate(model.auth)._tag === 'BrowserPermissionDenied') {
    return [InstallAccountMenuOutsideClick()]
  }

  if (model.route._tag === 'Onboarding') {
    return [
      InstallAccountMenuOutsideClick(),
      ...(model.auth.onboarding.step === 'repository' &&
      model.onboarding.repositories._tag === 'OnboardingRepositoriesIdle'
        ? [LoadOnboardingRepositories()]
        : []),
    ]
  }

  if (model.route._tag === 'Order') {
    return [InstallAccountMenuOutsideClick(), LoadCustomerOrders({})]
  }

  if (model.route._tag === 'OrderDetail') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadCustomerOrder({
        orderId: model.route.orderId,
      }),
    ]
  }

  if (model.route._tag === 'AutopilotWork') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadAutopilotWorkList({}),
      LoadAutopilotMorningReport({}),
    ]
  }

  if (model.route._tag === 'Forge') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadAutopilotWorkList({}),
      LoadAutopilotMorningReport({}),
      LoadCustomerOneCohort({}),
      LoadProviderAccountPool({}),
    ]
  }

  if (model.route._tag === 'Decisions') {
    return [InstallAccountMenuOutsideClick(), LoadAutopilotDecisions({})]
  }

  if (model.route._tag === 'Workspace') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadPrefilledWorkspace({ workspaceId: model.route.workspaceId }),
    ]
  }

  if (model.route._tag === 'Workroom' || model.route._tag === 'WorkroomTab') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadWorkroomSurface({ workroomId: model.route.workroomId }),
      LoadWorkroomLifecycle({ workroomId: model.route.workroomId }),
    ]
  }

  if (model.route._tag === 'AutopilotWorkDetail') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadAutopilotWorkDetail({ workOrderRef: model.route.workOrderRef }),
      LoadAutopilotWorkEvents({ workOrderRef: model.route.workOrderRef }),
      LoadAutopilotWorkBriefing({ workOrderRef: model.route.workOrderRef }),
    ]
  }

  if (model.route._tag === 'Admin') {
    return [InstallAccountMenuOutsideClick(), LoadAdminOverview()]
  }

  if (model.route._tag === 'Stats') {
    return [
      InstallAccountMenuOutsideClick(),
      LoadTokenUsageStats({ filters: model.tokenUsageStats.filters }),
    ]
  }

  if (model.route._tag === 'Mullet') {
    return [InstallAccountMenuOutsideClick(), LoadMulletBootstrap()]
  }

  const baseCommands = [
    LoadSyncSnapshot({
      href: syncSnapshotHref(model.sync.workspaceScope),
      scope: model.sync.workspaceScope,
    }),
  ]

  const fileRequest = threadFilesRequestForRoute(model)
  const fileCommands =
    fileRequest === undefined ? [] : [LoadThreadFiles(fileRequest)]
  const fileDetailRequest = threadFileDetailRequestForRoute(model)
  const fileDetailCommands =
    fileDetailRequest === undefined
      ? []
      : [LoadThreadFileDetail(fileDetailRequest)]
  const teamChatRequest = teamChatMessagesRequestForRoute(model)
  const teamChatCommands =
    teamChatRequest === undefined ? [] : [LoadTeamChatMessages(teamChatRequest)]
  const goalRequest = agentGoalScopeForRoute(model)
  const goalCommands =
    goalRequest === undefined ? [] : [LoadAgentGoal(goalRequest)]
  const chatCommands =
    model.route._tag === 'Chat' ||
    model.route._tag === 'TeamChat' ||
    model.route._tag === 'TeamProjectChat' ||
    model.route._tag === 'Thread'
      ? [FocusChatComposer()]
      : []
  const providerAccountPoolCommands =
    model.route._tag === 'SettingsSection' &&
    model.route.section === 'connections'
      ? [LoadProviderAccountPool({})]
      : []

  return [
    ...baseCommands,
    InstallAccountMenuOutsideClick(),
    ...artanisOperatorInitialCommands(model),
    ...goalCommands,
    ...teamChatCommands,
    ...fileCommands,
    ...fileDetailCommands,
    ...chatCommands,
    ...providerAccountPoolCommands,
    ...notificationInitialCommands(model),
  ]
}
