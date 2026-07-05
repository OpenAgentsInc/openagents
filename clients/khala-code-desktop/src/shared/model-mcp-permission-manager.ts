import type { KhalaCodeDesktopCodexEcosystemProjection } from "./codex-ecosystem"
import type {
  KhalaCodeDesktopCodexSettingsModelOption,
  KhalaCodeDesktopCodexSettingsPermissionProfile,
  KhalaCodeDesktopCodexSettingsProjection,
} from "./codex-settings"

export type KhalaCodeModelManagerEntry = Readonly<{
  id: string
  displayName: string
  providerId: string
  providerDisplayName: string
  selected: boolean
  hiddenByRuntime: boolean
  hiddenByUser: boolean
  visible: boolean
  state: "available" | "hidden" | "disabled" | "unpaid"
  detail: string
}>

export type KhalaCodeMcpManagerEntry = Readonly<{
  id: string
  name: string
  state: "connected" | "failed" | "needs_auth" | "needs_registration" | "disabled"
  enabled: boolean
  detail: string
  retryable: boolean
}>

export type KhalaCodePermissionAutoAcceptMode = "manual" | "session" | "directory"

export type KhalaCodePermissionManagerProfile = Readonly<{
  id: string
  label: string
  allowed: boolean
  selected: boolean
}>

export type KhalaCodePermissionAutoAcceptProjection = Readonly<{
  mode: KhalaCodePermissionAutoAcceptMode
  allowed: boolean
  detail: string
}>

export type KhalaCodeModelMcpPermissionManagerProjection = Readonly<{
  models: readonly KhalaCodeModelManagerEntry[]
  mcp: readonly KhalaCodeMcpManagerEntry[]
  permissions: Readonly<{
    profiles: readonly KhalaCodePermissionManagerProfile[]
    selectedProfile: string | null
    autoAccept: KhalaCodePermissionAutoAcceptProjection
  }>
}>

export type KhalaCodeMcpManagerIntent = Readonly<{
  ok: boolean
  serverId: string
  action: "enable" | "disable" | "authenticate" | "review"
  nextStep: "reload_mcp" | "oauth_login" | "runtime_registration" | "disabled" | "review_error"
  message: string
  retryable: boolean
}>

const stateForModel = (
  model: KhalaCodeDesktopCodexSettingsModelOption,
  hiddenByUser: boolean,
): KhalaCodeModelManagerEntry["state"] => {
  if (model.hidden || hiddenByUser) return "hidden"
  if (model.serviceTiers.some(tier => /paid|pro|team|enterprise|usage/i.test(`${tier.id} ${tier.name}`))) {
    return "unpaid"
  }
  return "available"
}

const modelDetail = (
  model: KhalaCodeDesktopCodexSettingsModelOption,
  state: KhalaCodeModelManagerEntry["state"],
): string => {
  if (state === "hidden") return "Hidden from composer and settings pickers."
  if (state === "unpaid") return "May require paid provider quota or service tier."
  if (model.supportedReasoningEfforts.length > 0) {
    return `${model.supportedReasoningEfforts.length} reasoning levels.`
  }
  return model.description ?? "Available from the current model catalog."
}

export const projectKhalaCodeModelManagerEntries = (
  settings: KhalaCodeDesktopCodexSettingsProjection,
  hiddenModelIds: ReadonlySet<string> = new Set(),
): readonly KhalaCodeModelManagerEntry[] =>
  settings.models.options.map(model => {
    const hiddenByUser = hiddenModelIds.has(model.id)
    const state = stateForModel(model, hiddenByUser)
    const providerId = model.providerId ?? "default"
    return {
      id: model.id,
      displayName: model.displayName,
      providerId,
      providerDisplayName: model.providerDisplayName ?? providerId,
      selected: settings.models.selected?.id === model.id,
      hiddenByRuntime: model.hidden,
      hiddenByUser,
      visible: state !== "hidden",
      state,
      detail: modelDetail(model, state),
    }
  }).sort((left, right) =>
    left.providerDisplayName.localeCompare(right.providerDisplayName) ||
    left.displayName.localeCompare(right.displayName) ||
    left.id.localeCompare(right.id)
  )

export const filterKhalaCodeModelManagerEntries = (
  models: readonly KhalaCodeModelManagerEntry[],
  searchTerm: string,
): readonly KhalaCodeModelManagerEntry[] => {
  const query = searchTerm.trim().toLowerCase()
  if (query.length === 0) return models
  return models.filter(model =>
    model.displayName.toLowerCase().includes(query) ||
    model.id.toLowerCase().includes(query) ||
    model.providerDisplayName.toLowerCase().includes(query)
  )
}

export const projectKhalaCodeMcpManagerEntries = (
  ecosystem: KhalaCodeDesktopCodexEcosystemProjection | null,
): readonly KhalaCodeMcpManagerEntry[] => {
  if (ecosystem === null) return []
  return ecosystem.sections.mcp.items.map(item => {
    const state: KhalaCodeMcpManagerEntry["state"] =
      item.state === "ready" ? "connected"
        : item.state === "auth_required" ? "needs_auth"
          : item.state === "install_required" ? "needs_registration"
            : item.state === "disabled" || item.state === "disabled_by_admin" ? "disabled"
              : item.state === "error" ? "failed"
                : "needs_registration"
    return {
      id: item.id,
      name: item.name,
      state,
      enabled: item.enabled !== false && state !== "disabled",
      detail: item.detail,
      retryable: state !== "disabled",
    }
  }).sort((left, right) => left.name.localeCompare(right.name))
}

const permissionProfileLabel = (
  profile: KhalaCodeDesktopCodexSettingsPermissionProfile,
): string => profile.description === null ? profile.id : `${profile.id} - ${profile.description}`

export const projectKhalaCodePermissionProfiles = (
  settings: KhalaCodeDesktopCodexSettingsProjection,
): readonly KhalaCodePermissionManagerProfile[] =>
  settings.permissions.profiles.map(profile => ({
    id: profile.id,
    label: permissionProfileLabel(profile),
    allowed: profile.allowed,
    selected: profile.selected || settings.permissions.selectedProfile === profile.id,
  }))

export const projectKhalaCodePermissionAutoAccept = (
  settings: KhalaCodeDesktopCodexSettingsProjection,
  mode: KhalaCodePermissionAutoAcceptMode,
): KhalaCodePermissionAutoAcceptProjection => {
  if (settings.requirements.managed) {
    return {
      mode,
      allowed: false,
      detail: "Managed Codex requirements own permission policy; auto-accept is disabled here.",
    }
  }
  if (settings.config.approvalPolicy === "never") {
    return {
      mode,
      allowed: false,
      detail: "Approval policy is never; no per-session auto-accept control is needed.",
    }
  }
  return {
    mode,
    allowed: true,
    detail: mode === "manual"
      ? "Manual approval is active for this session."
      : `${mode} auto-accept is a local session intent until the runtime exposes durable policy.`,
  }
}

export const projectKhalaCodeModelMcpPermissionManager = (
  input: {
    readonly ecosystem: KhalaCodeDesktopCodexEcosystemProjection | null
    readonly hiddenModelIds?: ReadonlySet<string>
    readonly permissionAutoAcceptMode?: KhalaCodePermissionAutoAcceptMode
    readonly settings: KhalaCodeDesktopCodexSettingsProjection
  },
): KhalaCodeModelMcpPermissionManagerProjection => ({
  models: projectKhalaCodeModelManagerEntries(input.settings, input.hiddenModelIds),
  mcp: projectKhalaCodeMcpManagerEntries(input.ecosystem),
  permissions: {
    profiles: projectKhalaCodePermissionProfiles(input.settings),
    selectedProfile: input.settings.permissions.selectedProfile,
    autoAccept: projectKhalaCodePermissionAutoAccept(
      input.settings,
      input.permissionAutoAcceptMode ?? "manual",
    ),
  },
})

export const khalaCodeMcpManagerIntent = (
  entry: KhalaCodeMcpManagerEntry,
  action: KhalaCodeMcpManagerIntent["action"],
): KhalaCodeMcpManagerIntent => {
  if (entry.state === "disabled") {
    return {
      ok: false,
      serverId: entry.id,
      action,
      nextStep: "disabled",
      message: `${entry.name} is disabled by the active runtime configuration.`,
      retryable: false,
    }
  }
  if (entry.state === "needs_auth" || action === "authenticate") {
    return {
      ok: false,
      serverId: entry.id,
      action,
      nextStep: "oauth_login",
      message: `${entry.name} needs MCP OAuth/login in the Codex-owned server flow.`,
      retryable: true,
    }
  }
  if (entry.state === "needs_registration") {
    return {
      ok: false,
      serverId: entry.id,
      action,
      nextStep: "runtime_registration",
      message: `${entry.name} needs MCP client/server registration before it can be toggled.`,
      retryable: true,
    }
  }
  if (entry.state === "failed") {
    return {
      ok: false,
      serverId: entry.id,
      action,
      nextStep: "review_error",
      message: `${entry.name} failed to start; review diagnostics before enabling it.`,
      retryable: true,
    }
  }
  return {
    ok: true,
    serverId: entry.id,
    action,
    nextStep: "reload_mcp",
    message: `${entry.name} can be ${action === "disable" ? "disabled" : "enabled"} via Codex MCP config reload.`,
    retryable: true,
  }
}
