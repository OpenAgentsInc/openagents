import type { KhalaCodeDesktopClaudeSettingsProjection } from "../shared/claude-settings"

export type ClaudeSettingsSectionHandle = {
  readonly refresh: () => Promise<void>
}

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

/**
 * Read-only settings metrics never say the bare, unexplained word "Unset" —
 * a null/undefined/empty value here means the field reflects a default, so
 * it says "Default" in plain language instead
 * (khala_code.settings.no_bare_unset_labels.v1).
 */
const compact = (value: unknown): string => {
  if (value === null || value === undefined || value === "") return "Default"
  if (Array.isArray(value)) return value.join(", ") || "Default"
  if (typeof value === "object") return "Available"
  return String(value)
}

const metric = (label: string, value: unknown): HTMLElement => {
  const item = el("div", "khala-settings-metric")
  item.append(
    el("span", "khala-settings-metric-label", label),
    el("span", "khala-settings-metric-value", compact(value)),
  )
  return item
}

export const mountClaudeSettingsSection = (
  container: HTMLElement,
  options: {
    readonly fetch: () => Promise<KhalaCodeDesktopClaudeSettingsProjection>
  },
): ClaudeSettingsSectionHandle => {
  let settings: KhalaCodeDesktopClaudeSettingsProjection | null = null

  const render = (): void => {
    const existing = container.querySelector(".khala-settings-section--claude")
    existing?.remove()
    const section = el("section", "khala-settings-section khala-settings-section--claude")
    section.append(el("h3", "khala-settings-section-title", "Claude"))
    if (settings === null) {
      section.append(el("div", "khala-settings-empty", "No Claude settings loaded"))
      container.append(section)
      return
    }
    if (!settings.ok) {
      const status = el("div", "khala-settings-status khala-settings-status--error")
      status.textContent = settings.errors.join("\n")
      section.append(status)
    }
    const selected = settings.models.selected
    section.append(
      metric("Model", selected?.displayName ?? settings.init.model),
      metric("Permission mode", settings.init.permissionMode),
      metric("Account", settings.account.email),
      metric("Organization", settings.account.organization),
      metric("Plan", settings.account.subscriptionType),
      metric("API provider", settings.account.apiProvider),
      metric("Supported models", settings.models.options.map(model => model.displayName)),
    )
    container.append(section)
  }

  return {
    async refresh() {
      settings = await options.fetch()
      render()
    },
  }
}
