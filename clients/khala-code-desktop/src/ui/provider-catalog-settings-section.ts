import {
  khalaCodeProviderConnectionIntent,
  projectKhalaCodeProviderCatalog,
  validateKhalaCodeOpenAiCompatibleProvider,
  type KhalaCodeCustomOpenAiCompatibleProvider,
  type KhalaCodeProviderCatalogEntry,
} from "../shared/provider-catalog"
import type { KhalaCodeDesktopCodexSettingsProjection } from "../shared/codex-settings"

export type KhalaCodeProviderCatalogSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeProviderCatalogSettingsSectionOptions = Readonly<{
  fetch: () => Promise<KhalaCodeDesktopCodexSettingsProjection>
  writeModelProvider: (providerId: string | null) => Promise<{
    readonly ok: boolean
    readonly settings?: KhalaCodeDesktopCodexSettingsProjection
    readonly error?: string
  }>
}>

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const csvModels = (value: string): readonly string[] =>
  value.split(",").map(item => item.trim()).filter(Boolean)

const statusText = (entry: KhalaCodeProviderCatalogEntry): string =>
  entry.state.replace(/_/g, " ")

export const mountKhalaCodeProviderCatalogSettingsSection = (
  options: KhalaCodeProviderCatalogSettingsSectionOptions,
): KhalaCodeProviderCatalogSettingsSectionHandle => {
  let settings: KhalaCodeDesktopCodexSettingsProjection | null = null
  let status = ""
  let customProviders: KhalaCodeCustomOpenAiCompatibleProvider[] = []
  let customInput = {
    id: "local-openai",
    displayName: "Local OpenAI-compatible",
    baseUrl: "http://localhost:11434/v1",
    modelIdsText: "local-model",
    apiKeyConfigured: false,
  }
  let sectionNode: HTMLElement | null = null

  const setStatus = (message: string): void => {
    status = message
    renderIntoCurrentSection()
  }

  const refresh = async (): Promise<void> => {
    try {
      settings = await options.fetch()
      setStatus("")
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error))
    }
    renderIntoCurrentSection()
  }

  const selectProvider = async (entry: KhalaCodeProviderCatalogEntry): Promise<void> => {
    const intent = khalaCodeProviderConnectionIntent(entry, "select")
    if (!intent.ok && intent.nextStep !== "upgrade_plan" && intent.nextStep !== "configure_environment") {
      setStatus(intent.message)
      return
    }
    const result = await options.writeModelProvider(entry.id)
    if (result.ok) {
      if (result.settings !== undefined) settings = result.settings
      setStatus(`Selected ${entry.displayName}.`)
      renderIntoCurrentSection()
      return
    }
    setStatus(result.error ?? `Failed to select ${entry.displayName}.`)
  }

  const disconnectProvider = async (entry: KhalaCodeProviderCatalogEntry): Promise<void> => {
    const intent = khalaCodeProviderConnectionIntent(entry, "disconnect")
    const result = await options.writeModelProvider(null)
    if (result.ok) {
      if (result.settings !== undefined) settings = result.settings
      setStatus(`${intent.message} Cleared model_provider.`)
      renderIntoCurrentSection()
      return
    }
    setStatus(result.error ?? `Failed to disconnect ${entry.displayName}.`)
  }

  const showIntent = (entry: KhalaCodeProviderCatalogEntry): void => {
    setStatus(khalaCodeProviderConnectionIntent(entry, "connect").message)
  }

  const renderEntry = (entry: KhalaCodeProviderCatalogEntry): HTMLElement => {
    const row = el("div", "khala-provider-catalog-row")
    row.dataset.providerId = entry.id
    row.dataset.state = entry.state
    row.dataset.selected = entry.selected ? "true" : "false"

    const label = el("div", "khala-provider-catalog-label")
    label.append(
      el("span", "khala-provider-catalog-name", entry.displayName),
      el("span", "khala-provider-catalog-meta", `${entry.id} / ${entry.modelCount} models / ${entry.source}`),
    )

    const state = el("span", "khala-provider-catalog-state", statusText(entry))
    const detail = el("span", "khala-provider-catalog-detail", entry.detail)

    const connect = el("button", "khala-provider-catalog-action", entry.selected ? "Current" : "Connect")
    connect.type = "button"
    connect.disabled = entry.selected || entry.state === "disabled"
    connect.addEventListener("click", () => {
      showIntent(entry)
      void selectProvider(entry)
    })

    const disconnect = el("button", "khala-provider-catalog-action", "Disconnect")
    disconnect.type = "button"
    disconnect.disabled = !entry.selected
    disconnect.addEventListener("click", () => {
      void disconnectProvider(entry)
    })

    row.append(label, state, detail, connect, disconnect)
    return row
  }

  const renderInput = (
    labelText: string,
    name: string,
    value: string,
    onInput: (value: string) => void,
  ): HTMLElement => {
    const label = el("label", "khala-provider-custom-field")
    const text = el("span", "khala-provider-custom-label", labelText)
    const input = el("input", "khala-settings-select")
    input.name = name
    input.type = "text"
    input.value = value
    input.addEventListener("input", () => onInput(input.value))
    label.append(text, input)
    return label
  }

  const renderCustomForm = (): HTMLElement => {
    const form = el("form", "khala-provider-custom-form")
    form.append(
      renderInput("Provider id", "custom-provider-id", customInput.id, value => {
        customInput = { ...customInput, id: value }
      }),
      renderInput("Display name", "custom-provider-name", customInput.displayName, value => {
        customInput = { ...customInput, displayName: value }
      }),
      renderInput("Base URL", "custom-provider-base-url", customInput.baseUrl, value => {
        customInput = { ...customInput, baseUrl: value }
      }),
      renderInput("Models", "custom-provider-models", customInput.modelIdsText, value => {
        customInput = { ...customInput, modelIdsText: value }
      }),
    )

    const apiKey = el("label", "khala-provider-custom-check")
    const checkbox = el("input")
    checkbox.type = "checkbox"
    checkbox.checked = customInput.apiKeyConfigured
    checkbox.addEventListener("change", () => {
      customInput = { ...customInput, apiKeyConfigured: checkbox.checked }
    })
    apiKey.append(checkbox, el("span", undefined, "API key configured in server/runtime"))

    const validate = el("button", "khala-provider-catalog-action", "Validate")
    validate.type = "submit"
    form.addEventListener("submit", event => {
      event.preventDefault()
      const result = validateKhalaCodeOpenAiCompatibleProvider({
        id: customInput.id,
        displayName: customInput.displayName,
        baseUrl: customInput.baseUrl,
        modelIds: csvModels(customInput.modelIdsText),
        apiKeyConfigured: customInput.apiKeyConfigured,
      })
      if (!result.ok) {
        setStatus(result.errors.join(" "))
        return
      }
      customProviders = [
        ...customProviders.filter(provider => provider.id !== result.provider.id),
        result.provider,
      ]
      setStatus(result.warnings[0] ?? `${result.provider.displayName} is valid.`)
      renderIntoCurrentSection()
    })
    form.append(apiKey, validate)
    return form
  }

  const renderInto = (section: HTMLElement): void => {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Provider Catalog"))
    if (status.length > 0) {
      section.append(el("div", "khala-keybindings-status khala-provider-catalog-status", status))
    }
    if (settings === null) {
      section.append(el("p", "khala-settings-empty", "Provider catalog has not been loaded yet."))
      return
    }
    const entries = projectKhalaCodeProviderCatalog(settings, customProviders)
    const list = el("div", "khala-provider-catalog-list")
    for (const entry of entries) list.append(renderEntry(entry))
    section.append(list, renderCustomForm())
  }

  function renderIntoCurrentSection(): void {
    if (sectionNode !== null) renderInto(sectionNode)
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-provider-catalog-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
