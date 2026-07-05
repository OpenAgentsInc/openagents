import type {
  KhalaCodeDesktopCodexSettingsProjection,
  KhalaCodeDesktopCodexSettingsProviderOption,
} from "./codex-settings"

export type KhalaCodeProviderConnectionState =
  | "connected"
  | "missing_auth"
  | "env_configured"
  | "paid"
  | "disabled"
  | "custom"

export type KhalaCodeProviderCatalogEntry = Readonly<{
  id: string
  displayName: string
  modelCount: number
  state: KhalaCodeProviderConnectionState
  selected: boolean
  source: "codex_settings" | "khala_custom"
  detail: string
  retryable: boolean
}>

export type KhalaCodeCustomOpenAiCompatibleProviderInput = Readonly<{
  id: string
  displayName: string
  baseUrl: string
  modelIds: readonly string[]
  apiKeyConfigured: boolean
}>

export type KhalaCodeCustomOpenAiCompatibleProvider = Readonly<{
  id: string
  displayName: string
  baseUrl: string
  modelIds: readonly string[]
  apiKeyConfigured: boolean
}>

export type KhalaCodeCustomOpenAiCompatibleProviderValidation = Readonly<
  | {
      ok: true
      provider: KhalaCodeCustomOpenAiCompatibleProvider
      warnings: readonly string[]
    }
  | {
      ok: false
      errors: readonly string[]
      warnings: readonly string[]
    }
>

export type KhalaCodeProviderConnectionIntent = Readonly<{
  ok: boolean
  providerId: string
  action: "connect" | "disconnect" | "select"
  nextStep:
    | "write_model_provider"
    | "open_codex_settings"
    | "configure_environment"
    | "upgrade_plan"
    | "custom_provider_pending_runtime"
    | "disabled"
  message: string
  retryable: boolean
}>

const PAID_PROVIDER_IDS = new Set(["openrouter", "anthropic", "google", "azure-openai"])

const ENV_CONFIGURED_PROVIDER_IDS = new Set([
  "ollama",
  "lmstudio",
  "openai-compatible",
  "vllm",
])

const CUSTOM_PROVIDER_ID_PATTERN = /^[a-z][a-z0-9_-]{1,62}$/

const scrubModelId = (value: string): string => value.trim()

const providerStateFor = (
  provider: KhalaCodeDesktopCodexSettingsProviderOption,
  selected: boolean,
): KhalaCodeProviderConnectionState => {
  if (selected) return "connected"
  if (provider.modelCount <= 0) return "disabled"
  if (ENV_CONFIGURED_PROVIDER_IDS.has(provider.id)) return "env_configured"
  if (PAID_PROVIDER_IDS.has(provider.id)) return "paid"
  return "missing_auth"
}

const providerDetailFor = (state: KhalaCodeProviderConnectionState): string => {
  switch (state) {
    case "connected":
      return "Selected for new Khala Code turns."
    case "env_configured":
      return "Available through local environment or server configuration."
    case "paid":
      return "May require paid account or quota before use."
    case "disabled":
      return "Disabled until the runtime reports available models."
    case "custom":
      return "Validated locally; runtime registration is handled outside the renderer."
    case "missing_auth":
      return "Needs provider authentication or server-side configuration."
  }
}

export const projectKhalaCodeProviderCatalog = (
  settings: KhalaCodeDesktopCodexSettingsProjection,
  customProviders: readonly KhalaCodeCustomOpenAiCompatibleProvider[] = [],
): readonly KhalaCodeProviderCatalogEntry[] => {
  const selectedProviderId = settings.config.modelProvider ?? settings.providers.selected?.id ?? null
  const entries: KhalaCodeProviderCatalogEntry[] = settings.providers.options.map(provider => {
    const selected = selectedProviderId === provider.id
    const state = providerStateFor(provider, selected)
    return {
      id: provider.id,
      displayName: provider.displayName,
      modelCount: provider.modelCount,
      state,
      selected,
      source: "codex_settings",
      detail: providerDetailFor(state),
      retryable: state !== "disabled",
    }
  })
  for (const provider of customProviders) {
    entries.push({
      id: provider.id,
      displayName: provider.displayName,
      modelCount: provider.modelIds.length,
      state: "custom",
      selected: selectedProviderId === provider.id,
      source: "khala_custom",
      detail: provider.apiKeyConfigured
        ? providerDetailFor("custom")
        : "Validated locally; add a server-side API key before routing turns.",
      retryable: true,
    })
  }
  return entries.sort((left, right) =>
    Number(right.selected) - Number(left.selected) ||
    left.displayName.localeCompare(right.displayName) ||
    left.id.localeCompare(right.id)
  )
}

export const validateKhalaCodeOpenAiCompatibleProvider = (
  input: KhalaCodeCustomOpenAiCompatibleProviderInput,
): KhalaCodeCustomOpenAiCompatibleProviderValidation => {
  const errors: string[] = []
  const warnings: string[] = []
  const id = input.id.trim().toLowerCase()
  const displayName = input.displayName.trim()
  const modelIds = [...new Set(input.modelIds.map(scrubModelId).filter(Boolean))]
  let baseUrl = input.baseUrl.trim()

  if (!CUSTOM_PROVIDER_ID_PATTERN.test(id)) {
    errors.push("Provider id must start with a letter and use 2-63 lowercase letters, numbers, underscores, or hyphens.")
  }
  if (displayName.length === 0) errors.push("Display name is required.")
  if (modelIds.length === 0) errors.push("At least one model id is required.")

  try {
    const parsed = new URL(baseUrl)
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      errors.push("Base URL must use http or https.")
    }
    parsed.username = ""
    parsed.password = ""
    parsed.hash = ""
    baseUrl = parsed.toString().replace(/\/$/, "")
  } catch {
    errors.push("Base URL must be a valid URL.")
  }

  if (!input.apiKeyConfigured) {
    warnings.push("API key is not stored in the renderer; configure it in the server/runtime before connecting.")
  }

  if (errors.length > 0) return { ok: false, errors, warnings }
  return {
    ok: true,
    provider: {
      id,
      displayName,
      baseUrl,
      modelIds,
      apiKeyConfigured: input.apiKeyConfigured,
    },
    warnings,
  }
}

export const khalaCodeProviderConnectionIntent = (
  entry: KhalaCodeProviderCatalogEntry,
  action: KhalaCodeProviderConnectionIntent["action"],
): KhalaCodeProviderConnectionIntent => {
  if (entry.state === "disabled") {
    return {
      ok: false,
      providerId: entry.id,
      action,
      nextStep: "disabled",
      message: `${entry.displayName} is disabled until models are available.`,
      retryable: false,
    }
  }
  if (action === "select") {
    return {
      ok: true,
      providerId: entry.id,
      action,
      nextStep: "write_model_provider",
      message: `${entry.displayName} can be selected with model_provider.`,
      retryable: true,
    }
  }
  if (entry.state === "paid") {
    return {
      ok: false,
      providerId: entry.id,
      action,
      nextStep: "upgrade_plan",
      message: `${entry.displayName} may require a paid provider account or quota.`,
      retryable: true,
    }
  }
  if (entry.state === "custom") {
    return {
      ok: true,
      providerId: entry.id,
      action,
      nextStep: "custom_provider_pending_runtime",
      message: `${entry.displayName} is validated; runtime registration remains server-owned.`,
      retryable: true,
    }
  }
  if (entry.state === "missing_auth") {
    return {
      ok: false,
      providerId: entry.id,
      action,
      nextStep: "configure_environment",
      message: `${entry.displayName} needs provider auth or environment configuration.`,
      retryable: true,
    }
  }
  return {
    ok: true,
    providerId: entry.id,
    action,
    nextStep: action === "disconnect" ? "open_codex_settings" : "write_model_provider",
    message: action === "disconnect"
      ? `${entry.displayName} can be disconnected from the server-owned provider settings.`
      : `${entry.displayName} is ready.`,
    retryable: true,
  }
}
