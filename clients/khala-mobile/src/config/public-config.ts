export type KhalaPublicConfig = Readonly<{
  appName: string
  appSlug: string
  appVersion: string
  iosBuildNumber: string | null
  androidVersionCode: number | null
  apiBaseUrl: string
  authBaseUrl: string
  syncBaseUrl: string
  updatesOwner: string
  updatesUrl: string
}>

export type KhalaPublicConfigSource = Readonly<{
  name?: unknown
  slug?: unknown
  version?: unknown
  extra?: unknown
  updates?: unknown
  ios?: unknown
  android?: unknown
}>

export type ExpoConstantsLike = Readonly<{
  expoConfig?: KhalaPublicConfigSource | null
}>

export class KhalaPublicConfigError extends Error {
  readonly issues: ReadonlyArray<string>

  constructor(issues: ReadonlyArray<string>) {
    super(`Invalid Khala public config: ${issues.join("; ")}`)
    this.name = "KhalaPublicConfigError"
    this.issues = issues
  }
}

const SECRET_KEY_PATTERN =
  /(api[_-]?key|access[_-]?token|auth[_-]?token|bearer|credential|mnemonic|password|private|secret|token)/i

const objectRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

const numberValue = (value: unknown): number | null =>
  typeof value === "number" && Number.isInteger(value) ? value : null

const httpsUrl = (value: unknown, label: string, issues: Array<string>): string => {
  const candidate = stringValue(value)
  if (candidate === null) {
    issues.push(`${label} must be a non-empty HTTPS URL`)
    return ""
  }

  try {
    const parsed = new URL(candidate)
    if (parsed.protocol !== "https:") {
      issues.push(`${label} must use https`)
      return ""
    }
    return parsed.toString().replace(/\/$/, "")
  } catch {
    issues.push(`${label} must be a valid URL`)
    return ""
  }
}

const collectSecretLikeKeys = (
  value: unknown,
  path: string,
  findings: Array<string>,
): void => {
  if (value === null || typeof value !== "object") return
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectSecretLikeKeys(item, `${path}[${index}]`, findings))
    return
  }

  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    const childPath = `${path}.${key}`
    if (SECRET_KEY_PATTERN.test(key)) {
      findings.push(childPath)
    }
    collectSecretLikeKeys(child, childPath, findings)
  }
}

/**
 * Bundled Expo config is public app metadata. Never place API keys, bearer
 * tokens, private endpoints, customer data, or other credential material in
 * `app.json` / `extra.khala`; secrets belong in SecureStore via
 * `src/security/keychain.ts`.
 */
export const parseKhalaPublicConfig = (
  source: KhalaPublicConfigSource,
): KhalaPublicConfig => {
  const issues: Array<string> = []
  const extra = objectRecord(source.extra)
  const khala = objectRecord(extra.khala)
  const updates = objectRecord(source.updates)
  const ios = objectRecord(source.ios)
  const android = objectRecord(source.android)

  const secretLikeKeys: Array<string> = []
  collectSecretLikeKeys(khala, "extra.khala", secretLikeKeys)
  if (secretLikeKeys.length > 0) {
    issues.push(
      `extra.khala is public and must not contain secret-shaped keys: ${secretLikeKeys.join(", ")}`,
    )
  }

  const appName = stringValue(source.name)
  const appSlug = stringValue(source.slug)
  const appVersion = stringValue(source.version)
  const iosBuildNumber = stringValue(ios.buildNumber)
  const androidVersionCode = numberValue(android.versionCode)
  const updatesOwner = stringValue(khala.updatesOwner)

  if (appName === null) issues.push("name must be a non-empty string")
  if (appSlug === null) issues.push("slug must be a non-empty string")
  if (appVersion === null) issues.push("version must be a non-empty string")
  if (updatesOwner === null) issues.push("extra.khala.updatesOwner must be a non-empty string")

  const apiBaseUrl = httpsUrl(khala.apiBaseUrl, "extra.khala.apiBaseUrl", issues)
  const authBaseUrl = httpsUrl(khala.authBaseUrl, "extra.khala.authBaseUrl", issues)
  const syncBaseUrl = httpsUrl(khala.syncBaseUrl, "extra.khala.syncBaseUrl", issues)
  const updatesUrl = httpsUrl(updates.url, "updates.url", issues)

  if (issues.length > 0) {
    throw new KhalaPublicConfigError(issues)
  }

  return {
    androidVersionCode,
    apiBaseUrl,
    appName: appName ?? "",
    appSlug: appSlug ?? "",
    appVersion: appVersion ?? "",
    authBaseUrl,
    iosBuildNumber,
    syncBaseUrl,
    updatesOwner: updatesOwner ?? "",
    updatesUrl,
  }
}

export const loadKhalaPublicConfig = (constants: ExpoConstantsLike): KhalaPublicConfig => {
  const expoConfig = constants.expoConfig ?? null
  if (expoConfig === null || expoConfig === undefined) {
    throw new KhalaPublicConfigError(["Expo Constants did not expose expoConfig"])
  }

  return parseKhalaPublicConfig(expoConfig)
}

export const loadKhalaPublicConfigFromExpo = async (): Promise<KhalaPublicConfig> => {
  const constantsModule = await import("expo-constants")
  return loadKhalaPublicConfig(constantsModule.default)
}
