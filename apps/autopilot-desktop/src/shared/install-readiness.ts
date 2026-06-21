import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
  NodeLaunchStatus,
} from "./rpc.js"

export type InstallReadinessStatus =
  | "ready"
  | "waiting"
  | "blocked"
  | "attention"

export type InstallReadinessItem = {
  readonly id: string
  readonly label: string
  readonly status: InstallReadinessStatus
  readonly detail: string
  readonly blockerRef: string | null
}

export type InstallReadinessInput = {
  readonly fetchedAt: string
  readonly platform: string
  readonly arch: string
  readonly runtime: "source" | "packaged"
  readonly nodeLaunchStatus: NodeLaunchStatus | null
  readonly pylonHomePresent: boolean
  readonly controlTokenPresent: boolean
  readonly builtInAgentReadiness: BuiltInAgentReadinessResponse
  readonly appleFmReadiness: AppleFmReadinessResponse | null
  readonly autoUpdateDisabledReason: string | null
}

export type InstallReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: "desktop:install-readiness"
  readonly platform: string
  readonly arch: string
  readonly runtime: "source" | "packaged"
  readonly nodeLaunchStatus: NodeLaunchStatus | null
  readonly pylonHomePresent: boolean
  readonly controlTokenPresent: boolean
  readonly localPylonReady: boolean
  readonly builtInAgentReady: boolean
  readonly appleFmReady: boolean
  readonly userApiKeyRequired: false
  readonly autoUpdateEnabled: boolean
  readonly highestRoiAction: string
  readonly blockerRefs: readonly string[]
  readonly items: readonly InstallReadinessItem[]
}

const nodeReadinessItem = (
  input: InstallReadinessInput,
): InstallReadinessItem => {
  const localPylonReady = input.pylonHomePresent && input.controlTokenPresent
  if (localPylonReady) {
    return {
      id: "local-pylon",
      label: "Local node",
      status: "ready",
      detail: "Pylon control is reachable over loopback.",
      blockerRef: null,
    }
  }

  switch (input.nodeLaunchStatus) {
    case "launching":
      return {
        id: "local-pylon",
        label: "Local node",
        status: "waiting",
        detail: "Autopilot is starting the bundled Pylon node.",
        blockerRef: "blocker.autopilot.install.local_pylon_launching",
      }
    case "failed":
      return {
        id: "local-pylon",
        label: "Local node",
        status: "blocked",
        detail: "The local Pylon node did not become reachable.",
        blockerRef: "blocker.autopilot.install.local_pylon_failed",
      }
    case "unavailable":
      return {
        id: "local-pylon",
        label: "Local node",
        status: "blocked",
        detail: "No bundled Pylon node entry was found in this app build.",
        blockerRef: "blocker.autopilot.install.pylon_bundle_missing",
      }
    case "adopted":
    case "online":
      return {
        id: "local-pylon",
        label: "Local node",
        status: "waiting",
        detail: "Node lifecycle is up, but the control token is not readable yet.",
        blockerRef: "blocker.autopilot.install.control_token_missing",
      }
    case null:
      return {
        id: "local-pylon",
        label: "Local node",
        status: "waiting",
        detail: "Waiting for first local-node status.",
        blockerRef: "blocker.autopilot.install.local_pylon_not_observed",
      }
  }
}

const builtInAgentReadinessItem = (
  readiness: BuiltInAgentReadinessResponse,
): InstallReadinessItem => {
  if (readiness.ok) {
    return {
      id: "builtin-agent",
      label: "Built-in agent",
      status: "ready",
      detail: `${readiness.meteringLabel}; no user API key required.`,
      blockerRef: null,
    }
  }

  if (!readiness.enabled) {
    return {
      id: "builtin-agent",
      label: "Built-in agent",
      status: "blocked",
      detail: "The built-in agent path is disabled for this build.",
      blockerRef: "blocker.autopilot.builtin_agent.disabled",
    }
  }

  if (!readiness.localPylonReady) {
    return {
      id: "builtin-agent",
      label: "Built-in agent",
      status: "waiting",
      detail: "Waiting for the local Pylon node before starting a hosted agent.",
      blockerRef: "blocker.autopilot.builtin_agent.local_pylon_offline",
    }
  }

  if (!readiness.hostedComputeConfigured) {
    return {
      id: "builtin-agent",
      label: "Built-in agent",
      status: "blocked",
      detail: "Hosted OpenAgents compute is not configured in this build.",
      blockerRef:
        "blocker.autopilot.builtin_agent.hosted_compute_unconfigured",
    }
  }

  const blockerRef =
    readiness.blockerRefs[0] ?? "blocker.autopilot.builtin_agent.blocked"
  return {
    id: "builtin-agent",
    label: "Built-in agent",
    status: "blocked",
    detail: blockerRef,
    blockerRef,
  }
}

const appleFmReadinessItem = (
  readiness: AppleFmReadinessResponse | null,
): InstallReadinessItem => {
  if (readiness === null) {
    return {
      id: "local-apple-fm",
      label: "Local Apple FM",
      status: "attention",
      detail: "Apple FM readiness has not been checked yet.",
      blockerRef: "blocker.autopilot.apple_fm.not_checked",
    }
  }

  if (readiness.ok) {
    return {
      id: "local-apple-fm",
      label: "Local Apple FM",
      status: "ready",
      detail: `Foundation Models ready through ${readiness.model}.`,
      blockerRef: null,
    }
  }

  if (!readiness.localPylonReady) {
    return {
      id: "local-apple-fm",
      label: "Local Apple FM",
      status: "attention",
      detail: "Waiting for local Pylon control before checking Apple FM.",
      blockerRef: "blocker.autopilot.apple_fm.local_pylon_offline",
    }
  }

  const blockerRef =
    readiness.blockerRefs[0] ??
    readiness.unavailableReason ??
    "blocker.autopilot.apple_fm.not_ready"
  const detail =
    readiness.unavailableReason === "unsupported_hardware"
      ? "This Mac does not support local Apple Foundation Models."
      : readiness.unavailableReason === "apple_intelligence_disabled"
        ? "Apple Intelligence is disabled or unavailable for this user."
        : readiness.unavailableReason === "bridge_unreachable"
          ? "The local Foundation Models bridge is not reachable."
          : readiness.message ?? `Apple FM is ${readiness.status}.`

  return {
    id: "local-apple-fm",
    label: "Local Apple FM",
    status: "attention",
    detail,
    blockerRef,
  }
}

const autoUpdateReadinessItem = (
  disabledReason: string | null,
): InstallReadinessItem => {
  if (disabledReason === null) {
    return {
      id: "auto-update",
      label: "Updates",
      status: "ready",
      detail: "Auto-update is enabled.",
      blockerRef: null,
    }
  }

  return {
    id: "auto-update",
    label: "Updates",
    status: "attention",
    detail: disabledReason,
    blockerRef: "blocker.autopilot.install.autoupdate_disabled",
  }
}

const highestRoiAction = (
  items: readonly InstallReadinessItem[],
  builtInAgentReady: boolean,
): string => {
  if (builtInAgentReady) return "Go online"
  const node = items.find(item => item.id === "local-pylon")
  if (node?.status === "waiting") return "Wait for local node"
  if (node?.status === "blocked") return "Restart Autopilot or install a newer build"
  const agent = items.find(item => item.id === "builtin-agent")
  if (agent?.blockerRef?.includes("hosted_compute_unconfigured")) {
    return "Install the hosted-compute desktop recut"
  }
  if (agent?.status === "waiting") return "Wait for local node"
  return "Open Settings"
}

export const projectInstallReadiness = (
  input: InstallReadinessInput,
): InstallReadinessResponse => {
  const items = [
    nodeReadinessItem(input),
    builtInAgentReadinessItem(input.builtInAgentReadiness),
    appleFmReadinessItem(input.appleFmReadiness),
    autoUpdateReadinessItem(input.autoUpdateDisabledReason),
  ] as const
  const blockingItems = items.filter(
    item => item.status === "blocked" || item.status === "waiting",
  )
  const blockerRefs = blockingItems.flatMap(item =>
    item.blockerRef === null ? [] : [item.blockerRef],
  )
  const builtInAgentReady = input.builtInAgentReadiness.ok
  const appleFmReady = input.appleFmReadiness?.ok ?? false
  const localPylonReady = input.pylonHomePresent && input.controlTokenPresent

  return {
    ok: blockerRefs.length === 0,
    fetchedAt: input.fetchedAt,
    sourceUrl: "desktop:install-readiness",
    platform: input.platform,
    arch: input.arch,
    runtime: input.runtime,
    nodeLaunchStatus: input.nodeLaunchStatus,
    pylonHomePresent: input.pylonHomePresent,
    controlTokenPresent: input.controlTokenPresent,
    localPylonReady,
    builtInAgentReady,
    appleFmReady,
    userApiKeyRequired: false,
    autoUpdateEnabled: input.autoUpdateDisabledReason === null,
    highestRoiAction: highestRoiAction(items, builtInAgentReady),
    blockerRefs,
    items,
  }
}
