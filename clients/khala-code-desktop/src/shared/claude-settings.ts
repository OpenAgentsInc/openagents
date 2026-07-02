export type KhalaCodeDesktopClaudeSettingsModel = {
  readonly description: string | null
  readonly displayName: string
  readonly selected: boolean
  readonly supportsAdaptiveThinking: boolean | null
  readonly supportsEffort: boolean | null
  readonly supportedEffortLevels: readonly string[]
  readonly value: string
}

export type KhalaCodeDesktopClaudeSettingsProjection = {
  readonly ok: boolean
  readonly observedAt: string
  readonly errors: readonly string[]
  readonly account: {
    readonly apiProvider: string | null
    readonly apiKeySource: string | null
    readonly email: string | null
    readonly organization: string | null
    readonly subscriptionType: string | null
    readonly tokenSource: string | null
  }
  readonly init: {
    readonly permissionMode: string | null
    readonly model: string | null
    readonly system: unknown
  }
  readonly models: {
    readonly options: readonly KhalaCodeDesktopClaudeSettingsModel[]
    readonly selected: KhalaCodeDesktopClaudeSettingsModel | null
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null

const booleanValue = (value: unknown): boolean | null =>
  typeof value === "boolean" ? value : null

const stringArray = (value: unknown): readonly string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []

const modelOptionsFrom = (
  supportedModels: unknown,
  selectedModel: string | null,
): readonly KhalaCodeDesktopClaudeSettingsModel[] =>
  (Array.isArray(supportedModels) ? supportedModels : [])
    .filter(isRecord)
    .map(model => {
      const value = stringValue(model.value) ?? stringValue(model.id) ?? stringValue(model.model) ?? ""
      return {
        description: stringValue(model.description),
        displayName: stringValue(model.displayName) ?? value,
        selected: selectedModel !== null && value === selectedModel,
        supportsAdaptiveThinking: booleanValue(model.supportsAdaptiveThinking),
        supportsEffort: booleanValue(model.supportsEffort),
        supportedEffortLevels: stringArray(model.supportedEffortLevels),
        value,
      }
    })
    .filter(model => model.value.length > 0)

export function projectKhalaCodeDesktopClaudeSettings(input: {
  readonly accountInfo?: unknown
  readonly errors?: readonly string[]
  readonly initializationResult?: unknown
  readonly observedAt?: string
  readonly permissionMode?: string | null
  readonly supportedModels?: unknown
}): KhalaCodeDesktopClaudeSettingsProjection {
  const init = isRecord(input.initializationResult) ? input.initializationResult : {}
  const account = isRecord(input.accountInfo) ? input.accountInfo : {}
  const selectedModel = stringValue(init.model) ?? stringValue(init.selectedModel)
  const models = modelOptionsFrom(input.supportedModels, selectedModel)
  const selected = models.find(model => model.selected) ?? models[0] ?? null
  const errors = input.errors ?? []
  return {
    ok: errors.length === 0,
    observedAt: input.observedAt ?? new Date().toISOString(),
    errors,
    account: {
      apiProvider: stringValue(account.apiProvider),
      apiKeySource: stringValue(account.apiKeySource),
      email: stringValue(account.email),
      organization: stringValue(account.organization),
      subscriptionType: stringValue(account.subscriptionType),
      tokenSource: stringValue(account.tokenSource),
    },
    init: {
      permissionMode: input.permissionMode ?? stringValue(init.permissionMode),
      model: selectedModel,
      system: init.system ?? init,
    },
    models: {
      options: models,
      selected,
    },
  }
}
