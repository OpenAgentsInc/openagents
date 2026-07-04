import {
  KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
  isKhalaCodeArchitectCoderJudgeRegistry,
  khalaCodeArchitectCoderJudgePreset,
  type KhalaCodeDesktopModelRolePreset,
} from "./model-role-preset.js"

export type KhalaCodeDesktopCodexJsonValue =
  | null
  | boolean
  | number
  | string
  | readonly KhalaCodeDesktopCodexJsonValue[]
  | { readonly [key: string]: KhalaCodeDesktopCodexJsonValue }

export type KhalaCodeDesktopCodexSettingsModelOption = {
  readonly id: string
  readonly model: string
  readonly displayName: string
  readonly description: string | null
  readonly hidden: boolean
  readonly isDefault: boolean
  readonly supportsPersonality: boolean
  readonly defaultReasoningEffort: string | null
  readonly supportedReasoningEfforts: readonly {
    readonly value: string
    readonly description: string | null
  }[]
  readonly serviceTiers: readonly {
    readonly id: string
    readonly name: string
    readonly description: string | null
  }[]
  readonly defaultServiceTier: string | null
}

export type KhalaCodeDesktopCodexSettingsProviderOption = {
  readonly id: string
  readonly displayName: string
  readonly modelCount: number
}

export type KhalaCodeDesktopCodexSettingsPermissionProfile = {
  readonly id: string
  readonly description: string | null
  readonly allowed: boolean
  readonly selected: boolean
}

export type KhalaCodeDesktopCodexSettingsCollaborationMode = {
  readonly name: string
  readonly mode: string | null
  readonly model: string | null
  readonly reasoningEffort: string | null
}

export type KhalaCodeDesktopCodexSettingsProjection = {
  readonly ok: boolean
  readonly observedAt: string
  readonly cwd: string | null
  readonly errors: readonly string[]
  readonly config: {
    readonly model: string | null
    readonly modelProvider: string | null
    readonly reasoningEffort: string | null
    readonly reasoningSummary: string | null
    readonly verbosity: string | null
    readonly serviceTier: string | null
    readonly approvalPolicy: unknown
    readonly approvalsReviewer: unknown
    readonly sandboxMode: string | null
    readonly defaultPermissions: string | null
    readonly webSearch: string | null
    readonly personality: string | null
    readonly layersAvailable: boolean
    readonly originKeys: readonly string[]
  }
  readonly appearance: {
    readonly keymap: KhalaCodeDesktopCodexJsonValue
    readonly keyPaths: {
      readonly keymap: "tui.keymap"
      readonly pet: "tui.pet"
      readonly petAnchor: "tui.pet_anchor"
      readonly personality: "personality"
      readonly statusLine: "tui.status_line"
      readonly statusLineUseColors: "tui.status_line_use_colors"
      readonly theme: "tui.theme"
      readonly vimModeDefault: "tui.vim_mode_default"
    }
    readonly pet: string | null
    readonly petAnchor: string | null
    readonly personality: string | null
    readonly statusLine: readonly string[] | null
    readonly statusLineUseColors: boolean | null
    readonly theme: string | null
    readonly vimModeDefault: boolean | null
  }
  readonly models: {
    readonly selected: KhalaCodeDesktopCodexSettingsModelOption | null
    readonly options: readonly KhalaCodeDesktopCodexSettingsModelOption[]
    readonly serviceTierCommands: readonly string[]
  }
  readonly providers: {
    readonly selected: KhalaCodeDesktopCodexSettingsProviderOption | null
    readonly options: readonly KhalaCodeDesktopCodexSettingsProviderOption[]
  }
  readonly providerCapabilities: {
    readonly namespaceTools: boolean | null
    readonly imageGeneration: boolean | null
    readonly webSearch: boolean | null
  }
  readonly permissions: {
    readonly selectedProfile: string | null
    readonly profiles: readonly KhalaCodeDesktopCodexSettingsPermissionProfile[]
    readonly blockedProfileIds: readonly string[]
  }
  readonly requirements: {
    readonly managed: boolean
    readonly allowedApprovalPolicies: readonly unknown[] | null
    readonly allowedSandboxModes: readonly string[] | null
    readonly allowedPermissionProfiles: readonly string[] | null
    readonly defaultPermissions: string | null
    readonly blockers: readonly {
      readonly key: string
      readonly message: string
    }[]
  }
  readonly usage: {
    readonly summary: unknown
    readonly dailyUsageBuckets: readonly unknown[] | null
    readonly available: boolean
  }
  readonly collaboration: {
    readonly modes: readonly KhalaCodeDesktopCodexSettingsCollaborationMode[]
    readonly currentMode: string | null
    readonly personality: string | null
  }
  readonly modelRolePresets: {
    readonly keyPath: typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH
    readonly activePreset: string | null
    readonly presets: readonly KhalaCodeDesktopModelRolePreset[]
  }
}

export type KhalaCodeDesktopCodexSettingsSource = {
  readonly cwd?: string | null
  readonly observedAt?: string
  readonly errors?: readonly string[]
  readonly configRead?: unknown
  readonly modelList?: unknown
  readonly providerCapabilities?: unknown
  readonly permissionProfileList?: unknown
  readonly requirementsRead?: unknown
  readonly usageRead?: unknown
  readonly collaborationModeList?: unknown
}

const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const optionalString = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null

const optionalBoolean = (value: unknown): boolean | null =>
  typeof value === "boolean" ? value : null

const arrayOfRecords = (value: unknown): readonly Record<string, unknown>[] =>
  Array.isArray(value)
    ? value.filter(item => item !== null && typeof item === "object" && !Array.isArray(item)) as Record<string, unknown>[]
    : []

const stringArray = (value: unknown): readonly string[] | null =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : null

const jsonValueOrNull = (value: unknown): KhalaCodeDesktopCodexJsonValue =>
  value === null ||
  typeof value === "boolean" ||
  typeof value === "number" ||
  typeof value === "string" ||
  Array.isArray(value) ||
  (typeof value === "object" && value !== null)
    ? value as KhalaCodeDesktopCodexJsonValue
    : null

const configFrom = (configRead: unknown): Record<string, unknown> =>
  asRecord(asRecord(configRead).config)

const configAt = (
  config: Record<string, unknown>,
  dottedPath: string,
): unknown => dottedPath.split(".").reduce<unknown>(
  (current, segment) => asRecord(current)[segment],
  config,
)

const originsFrom = (configRead: unknown): Record<string, unknown> =>
  asRecord(asRecord(configRead).origins)

const modelRecordsFrom = (
  modelList: unknown,
): readonly Record<string, unknown>[] => {
  const list = asRecord(modelList)
  const data = arrayOfRecords(list.data)
  return data.length > 0 ? data : arrayOfRecords(list.models)
}

const modelOptionsFrom = (
  modelList: unknown,
): readonly KhalaCodeDesktopCodexSettingsModelOption[] =>
  modelRecordsFrom(modelList).map(model => {
    const supportedReasoningEfforts = arrayOfRecords(model.supportedReasoningEfforts)
      .map(option => ({
        value: optionalString(option.reasoningEffort) ?? "",
        description: optionalString(option.description),
      }))
      .filter(option => option.value.length > 0)
    const serviceTiers = arrayOfRecords(model.serviceTiers)
      .map(tier => ({
        id: optionalString(tier.id) ?? "",
        name: optionalString(tier.name) ?? optionalString(tier.id) ?? "",
        description: optionalString(tier.description),
      }))
      .filter(tier => tier.id.length > 0)
    const id = optionalString(model.id) ?? optionalString(model.model) ?? ""
    return {
      id,
      model: optionalString(model.model) ?? id,
      displayName: optionalString(model.displayName) ?? optionalString(model.name) ?? id,
      description: optionalString(model.description),
      hidden: optionalBoolean(model.hidden) ?? false,
      isDefault: optionalBoolean(model.isDefault) ?? false,
      supportsPersonality: optionalBoolean(model.supportsPersonality) ?? false,
      defaultReasoningEffort: optionalString(model.defaultReasoningEffort),
      supportedReasoningEfforts,
      serviceTiers,
      defaultServiceTier: optionalString(model.defaultServiceTier),
    }
  }).filter(model => model.id.length > 0)

const providerIdFromModel = (
  model: Record<string, unknown>,
): string | null => {
  const provider = model.provider
  const providerRecord = asRecord(provider)
  return optionalString(provider) ??
    optionalString(model.providerId) ??
    optionalString(model.provider_id) ??
    optionalString(model.modelProvider) ??
    optionalString(model.model_provider) ??
    optionalString(providerRecord.id) ??
    optionalString(providerRecord.providerId) ??
    optionalString(providerRecord.name)
}

const providerDisplayNameFromModel = (
  model: Record<string, unknown>,
  providerId: string,
): string => {
  const providerRecord = asRecord(model.provider)
  return optionalString(model.providerDisplayName) ??
    optionalString(model.providerName) ??
    optionalString(model.provider_display_name) ??
    optionalString(model.provider_name) ??
    optionalString(providerRecord.displayName) ??
    optionalString(providerRecord.display_name) ??
    optionalString(providerRecord.name) ??
    providerId
}

const providerOptionsFrom = (
  modelList: unknown,
): readonly KhalaCodeDesktopCodexSettingsProviderOption[] => {
  const byId = new Map<string, KhalaCodeDesktopCodexSettingsProviderOption>()
  for (const model of modelRecordsFrom(modelList)) {
    const id = providerIdFromModel(model)
    if (id === null) continue
    const existing = byId.get(id)
    if (existing !== undefined) {
      byId.set(id, {
        ...existing,
        modelCount: existing.modelCount + 1,
      })
      continue
    }
    byId.set(id, {
      id,
      displayName: providerDisplayNameFromModel(model, id),
      modelCount: 1,
    })
  }
  return [...byId.values()]
}

const selectedModelFrom = (
  selected: string | null,
  options: readonly KhalaCodeDesktopCodexSettingsModelOption[],
): KhalaCodeDesktopCodexSettingsModelOption | null => {
  if (selected === null) {
    return options.find(model => model.isDefault) ?? options[0] ?? null
  }
  return options.find(model => model.id === selected || model.model === selected) ?? null
}

const selectedProviderFrom = (
  selected: string | null,
  options: readonly KhalaCodeDesktopCodexSettingsProviderOption[],
): KhalaCodeDesktopCodexSettingsProviderOption | null =>
  selected === null
    ? null
    : options.find(provider => provider.id === selected) ?? null

const permissionProfilesFrom = (
  permissionProfileList: unknown,
  selectedProfile: string | null,
): readonly KhalaCodeDesktopCodexSettingsPermissionProfile[] =>
  arrayOfRecords(asRecord(permissionProfileList).data).map(profile => {
    const id = optionalString(profile.id) ?? ""
    return {
      id,
      description: optionalString(profile.description),
      allowed: optionalBoolean(profile.allowed) ?? true,
      selected: selectedProfile !== null && selectedProfile === id,
    }
  }).filter(profile => profile.id.length > 0)

const collaborationModesFrom = (
  collaborationModeList: unknown,
): readonly KhalaCodeDesktopCodexSettingsCollaborationMode[] =>
  arrayOfRecords(asRecord(collaborationModeList).data).map(mode => ({
    name: optionalString(mode.name) ?? "",
    mode: optionalString(mode.mode),
    model: optionalString(mode.model),
    reasoningEffort: optionalString(mode.reasoning_effort),
  })).filter(mode => mode.name.length > 0)

const blockersFrom = (
  input: {
    readonly config: Record<string, unknown>
    readonly errors: readonly string[]
    readonly requirements: Record<string, unknown>
  },
): readonly { readonly key: string; readonly message: string }[] => {
  const blockers: { key: string; message: string }[] = input.errors.map((error, index) => ({
    key: `codex.settings.endpoint.${index}`,
    message: error,
  }))
  const allowedProfiles = asRecord(input.requirements.allowedPermissionProfiles)
  const selectedProfile = optionalString(input.config.default_permissions)
  if (
    selectedProfile !== null &&
    Object.keys(allowedProfiles).length > 0 &&
    allowedProfiles[selectedProfile] === false
  ) {
    blockers.push({
      key: "codex.settings.default_permissions.managed",
      message: `Permission profile ${selectedProfile} is blocked by Codex requirements.`,
    })
  }
  const allowedSandboxModes = stringArray(input.requirements.allowedSandboxModes)
  const sandboxMode = optionalString(input.config.sandbox_mode)
  if (
    sandboxMode !== null &&
    allowedSandboxModes !== null &&
    !allowedSandboxModes.includes(sandboxMode)
  ) {
    blockers.push({
      key: "codex.settings.sandbox_mode.managed",
      message: `Sandbox mode ${sandboxMode} is blocked by Codex requirements.`,
    })
  }
  return blockers
}

export const projectKhalaCodeDesktopCodexSettings = (
  input: KhalaCodeDesktopCodexSettingsSource,
): KhalaCodeDesktopCodexSettingsProjection => {
  const configRead = asRecord(input.configRead)
  const config = configFrom(configRead)
  const origins = originsFrom(configRead)
  const requirements = asRecord(asRecord(input.requirementsRead).requirements)
  const models = modelOptionsFrom(input.modelList)
  const selectedModel = selectedModelFrom(optionalString(config.model), models)
  const providers = providerOptionsFrom(input.modelList)
  const selectedProvider = selectedProviderFrom(optionalString(config.model_provider), providers)
  const selectedProfile = optionalString(config.default_permissions) ??
    optionalString(requirements.defaultPermissions)
  const profiles = permissionProfilesFrom(input.permissionProfileList, selectedProfile)
  const allowedPermissionProfiles = asRecord(requirements.allowedPermissionProfiles)
  const providerCapabilities = asRecord(input.providerCapabilities)
  const usageRead = asRecord(input.usageRead)
  const errors = input.errors ?? []
  const blockers = blockersFrom({ config, errors, requirements })
  const keyPaths = {
    keymap: "tui.keymap",
    pet: "tui.pet",
    petAnchor: "tui.pet_anchor",
    personality: "personality",
    statusLine: "tui.status_line",
    statusLineUseColors: "tui.status_line_use_colors",
    theme: "tui.theme",
    vimModeDefault: "tui.vim_mode_default",
  } as const
  const configuredRoleRegistry = configAt(config, KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH)
  const architectCoderJudgeSelected = isKhalaCodeArchitectCoderJudgeRegistry(configuredRoleRegistry)

  return {
    ok: errors.length === 0,
    observedAt: input.observedAt ?? new Date().toISOString(),
    cwd: input.cwd ?? null,
    errors,
    config: {
      model: optionalString(config.model),
      modelProvider: optionalString(config.model_provider),
      reasoningEffort: optionalString(config.model_reasoning_effort),
      reasoningSummary: optionalString(config.model_reasoning_summary),
      verbosity: optionalString(config.model_verbosity),
      serviceTier: optionalString(config.service_tier),
      approvalPolicy: config.approval_policy ?? null,
      approvalsReviewer: config.approvals_reviewer ?? null,
      sandboxMode: optionalString(config.sandbox_mode),
      defaultPermissions: selectedProfile,
      webSearch: optionalString(config.web_search),
      personality: optionalString(config.personality),
      layersAvailable: Array.isArray(configRead.layers),
      originKeys: Object.keys(origins).sort(),
    },
    appearance: {
      keymap: jsonValueOrNull(configAt(config, keyPaths.keymap)),
      keyPaths,
      pet: optionalString(configAt(config, keyPaths.pet)),
      petAnchor: optionalString(configAt(config, keyPaths.petAnchor)),
      personality: optionalString(config.personality),
      statusLine: stringArray(configAt(config, keyPaths.statusLine)),
      statusLineUseColors: optionalBoolean(configAt(config, keyPaths.statusLineUseColors)),
      theme: optionalString(configAt(config, keyPaths.theme)),
      vimModeDefault: optionalBoolean(configAt(config, keyPaths.vimModeDefault)),
    },
    models: {
      selected: selectedModel,
      options: models,
      serviceTierCommands: selectedModel === null
        ? []
        : selectedModel.serviceTiers.map(tier => tier.id),
    },
    providers: {
      selected: selectedProvider,
      options: providers,
    },
    providerCapabilities: {
      namespaceTools: optionalBoolean(providerCapabilities.namespaceTools),
      imageGeneration: optionalBoolean(providerCapabilities.imageGeneration),
      webSearch: optionalBoolean(providerCapabilities.webSearch),
    },
    permissions: {
      selectedProfile,
      profiles,
      blockedProfileIds: Object.entries(allowedPermissionProfiles)
        .filter(([, allowed]) => allowed === false)
        .map(([id]) => id)
        .sort(),
    },
    requirements: {
      managed: Object.keys(requirements).length > 0,
      allowedApprovalPolicies: Array.isArray(requirements.allowedApprovalPolicies)
        ? requirements.allowedApprovalPolicies
        : null,
      allowedSandboxModes: stringArray(requirements.allowedSandboxModes),
      allowedPermissionProfiles: Object.entries(allowedPermissionProfiles)
        .filter(([, allowed]) => allowed === true)
        .map(([id]) => id)
        .sort(),
      defaultPermissions: optionalString(requirements.defaultPermissions),
      blockers,
    },
    usage: {
      summary: usageRead.summary ?? null,
      dailyUsageBuckets: Array.isArray(usageRead.dailyUsageBuckets)
        ? usageRead.dailyUsageBuckets
        : null,
      available: usageRead.summary !== undefined,
    },
    collaboration: {
      modes: collaborationModesFrom(input.collaborationModeList),
      currentMode: optionalString(asRecord(config.collaboration_mode).mode) ??
        optionalString(config.mode),
      personality: optionalString(config.personality),
    },
    modelRolePresets: {
      keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
      activePreset: architectCoderJudgeSelected ? "architect-coder-judge" : null,
      presets: [khalaCodeArchitectCoderJudgePreset(architectCoderJudgeSelected)],
    },
  }
}
