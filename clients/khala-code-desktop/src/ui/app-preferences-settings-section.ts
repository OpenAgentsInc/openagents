import {
  applyKhalaCodeAppPreferences,
  defaultKhalaCodeAppPreferences,
  updateKhalaCodeAppPreference,
  type KhalaCodeAppPreferences,
  type KhalaCodeFontPreference,
} from "../shared/app-preferences"

export type KhalaCodeAppPreferencesSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => void
}>

export type KhalaCodeAppPreferencesSettingsSectionOptions = Readonly<{
  apply: (preferences: KhalaCodeAppPreferences) => void
  read: () => KhalaCodeAppPreferences
  reset: () => KhalaCodeAppPreferences
  write: (preferences: KhalaCodeAppPreferences) => void
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

const titleFor = (value: string): string =>
  value.replace(/([A-Z])/g, " $1").replace(/^./, char => char.toUpperCase())

export const mountKhalaCodeAppPreferencesSettingsSection = (
  options: KhalaCodeAppPreferencesSettingsSectionOptions,
): KhalaCodeAppPreferencesSettingsSectionHandle => {
  let preferences = options.read()
  let status = ""
  let sectionNode: HTMLElement | null = null

  const renderIntoCurrentSection = (): void => {
    if (sectionNode !== null) renderInto(sectionNode)
  }

  const persist = (
    next: KhalaCodeAppPreferences,
    message: string,
  ): void => {
    preferences = next
    options.write(preferences)
    options.apply(preferences)
    status = message
    renderIntoCurrentSection()
  }

  const update = (
    keyPath: string,
    value: string | boolean | number,
  ): void => {
    persist(
      updateKhalaCodeAppPreference(preferences, keyPath, value),
      `Saved ${keyPath}.`,
    )
  }

  const select = (
    labelText: string,
    keyPath: "codeFont" | "colorScheme" | "terminalFont" | "uiFont",
    current: string,
    values: readonly string[],
  ): HTMLElement => {
    const label = el("label", "khala-preferences-control")
    label.append(el("span", "khala-preferences-label", labelText))
    const input = el("select", "khala-settings-select")
    input.name = `app-preference-${keyPath}`
    for (const value of values) {
      const option = el("option")
      option.value = value
      option.textContent = titleFor(value)
      option.selected = value === current
      input.append(option)
    }
    input.addEventListener("change", () => update(keyPath, input.value))
    label.append(input)
    return label
  }

  const toggle = (
    labelText: string,
    keyPath: string,
    checked: boolean,
  ): HTMLElement => {
    const label = el("label", "khala-preferences-toggle")
    const input = el("input")
    input.type = "checkbox"
    input.name = `app-preference-${keyPath}`
    input.checked = checked
    input.addEventListener("change", () => update(keyPath, input.checked))
    label.append(input, el("span", undefined, labelText))
    return label
  }

  const group = (
    title: string,
    children: readonly HTMLElement[],
  ): HTMLElement => {
    const wrap = el("div", "khala-preferences-group")
    wrap.append(el("div", "khala-preferences-group-title", title), ...children)
    return wrap
  }

  const fontValues: readonly KhalaCodeFontPreference[] = ["default", "system", "mono", "serif"]

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "App Preferences"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-preferences-status", status))
    section.append(
      group("Theme And Fonts", [
        select("Color scheme", "colorScheme", preferences.colorScheme, ["khala", "system", "light"]),
        select("UI font", "uiFont", preferences.uiFont, fontValues),
        select("Code font", "codeFont", preferences.codeFont, fontValues),
        select("Terminal font", "terminalFont", preferences.terminalFont, fontValues),
      ]),
      group("Notifications", [
        toggle("Agent events", "notifications.agentEvents", preferences.notifications.agentEvents),
        toggle("Permission events", "notifications.permissionEvents", preferences.notifications.permissionEvents),
        toggle("Errors", "notifications.errors", preferences.notifications.errors),
        toggle("Completions", "notifications.completions", preferences.notifications.completions),
      ]),
      group("Sounds", [
        toggle("Agent events", "sounds.agentEvents", preferences.sounds.agentEvents),
        toggle("Permission events", "sounds.permissionEvents", preferences.sounds.permissionEvents),
        toggle("Errors", "sounds.errors", preferences.sounds.errors),
        toggle("Completions", "sounds.completions", preferences.sounds.completions),
        (() => {
          const label = el("label", "khala-preferences-control")
          label.append(el("span", "khala-preferences-label", "Volume"))
          const input = el("input", "khala-settings-select")
          input.type = "range"
          input.name = "app-preference-sounds.volume"
          input.min = "0"
          input.max = "1"
          input.step = "0.05"
          input.value = String(preferences.sounds.volume)
          input.addEventListener("change", () => update("sounds.volume", Number(input.value)))
          label.append(input)
          return label
        })(),
      ]),
      group("Layout Toggles", [
        toggle("Compact composer", "features.compactComposer", preferences.features.compactComposer),
        toggle("Dense workbench", "features.denseWorkbench", preferences.features.denseWorkbench),
        toggle("Provider diagnostics", "features.providerDiagnostics", preferences.features.providerDiagnostics),
        toggle("Terminal tabs", "features.terminalTabs", preferences.features.terminalTabs),
      ]),
    )

    const reset = el("button", "khala-settings-refresh khala-preferences-reset", "Reset")
    reset.type = "button"
    reset.addEventListener("click", () => {
      preferences = options.reset()
      options.apply(preferences)
      status = "Reset app preferences to defaults."
      renderIntoCurrentSection()
    })
    const defaults = defaultKhalaCodeAppPreferences()
    const summary = el(
      "p",
      "khala-preferences-summary",
      `Defaults keep ${defaults.colorScheme} colors, current UI/code fonts, notifications on, and sounds off.`,
    )
    section.append(reset, summary)
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-preferences-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return {
    render,
    refresh() {
      preferences = options.read()
      renderIntoCurrentSection()
    },
  }
}

export const applyKhalaCodeAppPreferencesToDocument = (
  preferences: KhalaCodeAppPreferences,
): void => {
  applyKhalaCodeAppPreferences(document.documentElement, preferences)
}
