export const KHALA_CODE_APP_PREFERENCES_STORAGE_KEY =
  "khala-code-desktop.app-preferences.v1"

export type KhalaCodeColorSchemePreference = "khala" | "system" | "light"
export type KhalaCodeFontPreference = "default" | "system" | "mono" | "serif"

export type KhalaCodeAppPreferences = Readonly<{
  colorScheme: KhalaCodeColorSchemePreference
  uiFont: KhalaCodeFontPreference
  codeFont: KhalaCodeFontPreference
  terminalFont: KhalaCodeFontPreference
  notifications: Readonly<{
    agentEvents: boolean
    permissionEvents: boolean
    errors: boolean
    completions: boolean
  }>
  sounds: Readonly<{
    agentEvents: boolean
    permissionEvents: boolean
    errors: boolean
    completions: boolean
    volume: number
  }>
  features: Readonly<{
    compactComposer: boolean
    denseWorkbench: boolean
    providerDiagnostics: boolean
    terminalTabs: boolean
  }>
}>

export const defaultKhalaCodeAppPreferences = (): KhalaCodeAppPreferences => ({
  colorScheme: "khala",
  uiFont: "default",
  codeFont: "default",
  terminalFont: "default",
  notifications: {
    agentEvents: true,
    permissionEvents: true,
    errors: true,
    completions: true,
  },
  sounds: {
    agentEvents: false,
    permissionEvents: false,
    errors: false,
    completions: false,
    volume: 0.4,
  },
  features: {
    compactComposer: false,
    denseWorkbench: false,
    providerDiagnostics: true,
    terminalTabs: false,
  },
})

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const booleanValue = (value: unknown, fallback: boolean): boolean =>
  typeof value === "boolean" ? value : fallback

const numberValue = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback

const enumValue = <Value extends string>(
  value: unknown,
  allowed: readonly Value[],
  fallback: Value,
): Value => typeof value === "string" && allowed.includes(value as Value)
  ? value as Value
  : fallback

const clampVolume = (value: number): number =>
  Math.min(1, Math.max(0, Math.round(value * 100) / 100))

export const parseKhalaCodeAppPreferences = (
  value: unknown,
): KhalaCodeAppPreferences => {
  const defaults = defaultKhalaCodeAppPreferences()
  if (!isRecord(value)) return defaults
  const notifications = isRecord(value.notifications) ? value.notifications : {}
  const sounds = isRecord(value.sounds) ? value.sounds : {}
  const features = isRecord(value.features) ? value.features : {}
  return {
    colorScheme: enumValue(value.colorScheme, ["khala", "system", "light"], defaults.colorScheme),
    uiFont: enumValue(value.uiFont, ["default", "system", "mono", "serif"], defaults.uiFont),
    codeFont: enumValue(value.codeFont, ["default", "system", "mono", "serif"], defaults.codeFont),
    terminalFont: enumValue(value.terminalFont, ["default", "system", "mono", "serif"], defaults.terminalFont),
    notifications: {
      agentEvents: booleanValue(notifications.agentEvents, defaults.notifications.agentEvents),
      permissionEvents: booleanValue(notifications.permissionEvents, defaults.notifications.permissionEvents),
      errors: booleanValue(notifications.errors, defaults.notifications.errors),
      completions: booleanValue(notifications.completions, defaults.notifications.completions),
    },
    sounds: {
      agentEvents: booleanValue(sounds.agentEvents, defaults.sounds.agentEvents),
      permissionEvents: booleanValue(sounds.permissionEvents, defaults.sounds.permissionEvents),
      errors: booleanValue(sounds.errors, defaults.sounds.errors),
      completions: booleanValue(sounds.completions, defaults.sounds.completions),
      volume: clampVolume(numberValue(sounds.volume, defaults.sounds.volume)),
    },
    features: {
      compactComposer: booleanValue(features.compactComposer, defaults.features.compactComposer),
      denseWorkbench: booleanValue(features.denseWorkbench, defaults.features.denseWorkbench),
      providerDiagnostics: booleanValue(features.providerDiagnostics, defaults.features.providerDiagnostics),
      terminalTabs: booleanValue(features.terminalTabs, defaults.features.terminalTabs),
    },
  }
}

export const readKhalaCodeAppPreferences = (
  storage: Storage,
): KhalaCodeAppPreferences => {
  const raw = storage.getItem(KHALA_CODE_APP_PREFERENCES_STORAGE_KEY)
  if (raw === null) return defaultKhalaCodeAppPreferences()
  try {
    return parseKhalaCodeAppPreferences(JSON.parse(raw) as unknown)
  } catch {
    return defaultKhalaCodeAppPreferences()
  }
}

export const writeKhalaCodeAppPreferences = (
  storage: Storage,
  preferences: KhalaCodeAppPreferences,
): void => {
  storage.setItem(
    KHALA_CODE_APP_PREFERENCES_STORAGE_KEY,
    JSON.stringify(preferences, null, 2),
  )
}

export const resetKhalaCodeAppPreferences = (
  storage: Storage,
): KhalaCodeAppPreferences => {
  storage.removeItem(KHALA_CODE_APP_PREFERENCES_STORAGE_KEY)
  return defaultKhalaCodeAppPreferences()
}

const fontFamilyFor = (
  preference: KhalaCodeFontPreference,
  kind: "code" | "terminal" | "ui",
): string => {
  if (preference === "system") return "-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
  if (preference === "mono") return "SFMono-Regular, ui-monospace, Menlo, monospace"
  if (preference === "serif") return "ui-serif, Georgia, serif"
  if (kind === "ui") return "var(--oa-font-sans)"
  return "var(--oa-font-code)"
}

export const applyKhalaCodeAppPreferences = (
  root: HTMLElement,
  preferences: KhalaCodeAppPreferences,
): void => {
  root.dataset.khalaColorScheme = preferences.colorScheme
  root.dataset.khalaFeatureCompactComposer = String(preferences.features.compactComposer)
  root.dataset.khalaFeatureDenseWorkbench = String(preferences.features.denseWorkbench)
  root.dataset.khalaFeatureProviderDiagnostics = String(preferences.features.providerDiagnostics)
  root.dataset.khalaFeatureTerminalTabs = String(preferences.features.terminalTabs)
  root.style.setProperty("--khala-ui-font-family", fontFamilyFor(preferences.uiFont, "ui"))
  root.style.setProperty("--khala-code-font-family", fontFamilyFor(preferences.codeFont, "code"))
  root.style.setProperty("--khala-terminal-font-family", fontFamilyFor(preferences.terminalFont, "terminal"))
}

export const updateKhalaCodeAppPreference = (
  preferences: KhalaCodeAppPreferences,
  keyPath: string,
  value: string | boolean | number,
): KhalaCodeAppPreferences => {
  if (keyPath === "colorScheme") {
    return {
      ...preferences,
      colorScheme: enumValue(value, ["khala", "system", "light"], preferences.colorScheme),
    }
  }
  if (keyPath === "uiFont" || keyPath === "codeFont" || keyPath === "terminalFont") {
    return {
      ...preferences,
      [keyPath]: enumValue(value, ["default", "system", "mono", "serif"], preferences[keyPath]),
    }
  }
  const [group, key] = keyPath.split(".")
  if (group === "notifications" && key in preferences.notifications) {
    return {
      ...preferences,
      notifications: {
        ...preferences.notifications,
        [key]: Boolean(value),
      },
    }
  }
  if (group === "sounds" && key in preferences.sounds) {
    return {
      ...preferences,
      sounds: {
        ...preferences.sounds,
        [key]: key === "volume" ? clampVolume(Number(value)) : Boolean(value),
      },
    }
  }
  if (group === "features" && key in preferences.features) {
    return {
      ...preferences,
      features: {
        ...preferences.features,
        [key]: Boolean(value),
      },
    }
  }
  return preferences
}
