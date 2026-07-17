export const MOBILE_ENVIRONMENT_MAX_ROWS = 50
export const MOBILE_PAIRING_CODE_MAX = 96
export const MOBILE_SHARE_TEXT_MAX = 20_000

export type MobileEnvironmentHealth = "connected" | "degraded" | "offline"
export type MobileEnvironmentKind = "owner_local" | "cloud"

export type MobileEnvironmentRow = Readonly<{
  environmentRef: string
  label: string
  kind: MobileEnvironmentKind
  health: MobileEnvironmentHealth
  paired: boolean
  capabilities: ReadonlyArray<"coding" | "git" | "terminal" | "notifications">
  lastSeenAt: string | null
  detail: string
}>

export type MobileEnvironmentDirectory = Readonly<{
  directoryRef: string
  environments: ReadonlyArray<MobileEnvironmentRow>
  truncated: boolean
}>

export type MobileEnvironmentMutationReceipt = Readonly<{
  receiptRef: string
  operation: "pair" | "reconnect"
  environmentRef: string
  recordedAt: string
  summary: string
  directory: MobileEnvironmentDirectory
}>

export type MobileEnvironmentConnectionsPort = Readonly<{
  environmentDirectory: () => Promise<unknown>
  pairEnvironment: (request: Readonly<{ pairingCode: string; idempotencyRef: string }>) => Promise<unknown>
  reconnectEnvironment: (request: Readonly<{
    environmentRef: string
    directoryRef: string
    idempotencyRef: string
  }>) => Promise<unknown>
}>

export type MobileNotificationPreferences = Readonly<{
  attention: boolean
  completion: boolean
  approvals: boolean
}>

export type MobileNotificationSnapshot = Readonly<{
  permission: "undetermined" | "denied" | "granted"
  registration: "unavailable" | "unregistered" | "registered"
  preferences: MobileNotificationPreferences
  detail: string
}>

export type MobileNotificationSettingsPort = Readonly<{
  snapshot: () => Promise<MobileNotificationSnapshot>
  requestPermission: () => Promise<MobileNotificationSnapshot>
  setPreferences: (preferences: MobileNotificationPreferences) => Promise<MobileNotificationSnapshot>
}>

export type MobileShareIntake = Readonly<{
  title: string | null
  text: string
  url: string | null
}>

export type MobileSettingsSection =
  | "root"
  | "account"
  | "environments"
  | "notifications"
  | "appearance"
  | "accessibility"
  | "storage"
  | "diagnostics"
  | "legal"
  | "share"

export type MobileSettingsState = Readonly<{
  section: MobileSettingsSection
  environments: MobileEnvironmentDirectory | null
  environmentState: "idle" | "loading" | "ready" | "unavailable"
  pairingCode: string
  submittingEnvironment: boolean
  selectedEnvironmentRef: string | null
  environmentReceipt: MobileEnvironmentMutationReceipt | null
  notification: MobileNotificationSnapshot
  notificationLoading: boolean
  incomingShare: MobileShareIntake | null
  notice: string | null
}>

export const defaultMobileNotificationPreferences: MobileNotificationPreferences = {
  attention: true,
  completion: true,
  approvals: true,
}

export const initialMobileSettingsState: MobileSettingsState = {
  section: "root",
  environments: null,
  environmentState: "idle",
  pairingCode: "",
  submittingEnvironment: false,
  selectedEnvironmentRef: null,
  environmentReceipt: null,
  notification: {
    permission: "undetermined",
    registration: "unregistered",
    preferences: defaultMobileNotificationPreferences,
    detail: "Notification permission has not been checked on this device.",
  },
  notificationLoading: false,
  incomingShare: null,
  notice: null,
}

const safeRef = (value: unknown): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= 256 && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)
const safeText = (value: unknown, max: number): value is string => typeof value === "string" &&
  value.length > 0 && value.length <= max && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(value)
const iso = (value: unknown): value is string => typeof value === "string" && /^\d{4}-\d{2}-\d{2}T/u.test(value)
const capabilities = new Set(["coding", "git", "terminal", "notifications"])

export const decodeMobileEnvironmentDirectory = (value: unknown): MobileEnvironmentDirectory | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!safeRef(row.directoryRef) || !Array.isArray(row.environments) ||
    row.environments.length > MOBILE_ENVIRONMENT_MAX_ROWS || typeof row.truncated !== "boolean") return null
  const seen = new Set<string>()
  const environments: MobileEnvironmentRow[] = []
  for (const candidate of row.environments) {
    if (typeof candidate !== "object" || candidate === null) return null
    const environment = candidate as Record<string, unknown>
    if (!safeRef(environment.environmentRef) || seen.has(environment.environmentRef) ||
      !safeText(environment.label, 120) ||
      (environment.kind !== "owner_local" && environment.kind !== "cloud") ||
      (environment.health !== "connected" && environment.health !== "degraded" && environment.health !== "offline") ||
      typeof environment.paired !== "boolean" || !Array.isArray(environment.capabilities) ||
      environment.capabilities.length > 4 || new Set(environment.capabilities).size !== environment.capabilities.length ||
      !environment.capabilities.every(item => typeof item === "string" && capabilities.has(item)) ||
      !(environment.lastSeenAt === null || iso(environment.lastSeenAt)) || !safeText(environment.detail, 300)) return null
    seen.add(environment.environmentRef)
    environments.push(environment as MobileEnvironmentRow)
  }
  return { directoryRef: row.directoryRef, environments, truncated: row.truncated }
}

export const decodeMobileEnvironmentReceipt = (
  value: unknown,
  operation: "pair" | "reconnect",
): MobileEnvironmentMutationReceipt | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  const directory = decodeMobileEnvironmentDirectory(row.directory)
  if (row.ok !== true || row.operation !== operation || !safeRef(row.receiptRef) ||
    !safeRef(row.environmentRef) || !iso(row.recordedAt) || !safeText(row.summary, 300) || directory === null ||
    !directory.environments.some(item => item.environmentRef === row.environmentRef)) return null
  return {
    receiptRef: row.receiptRef,
    operation,
    environmentRef: row.environmentRef,
    recordedAt: row.recordedAt,
    summary: row.summary,
    directory,
  }
}

export const normalizeMobilePairingCode = (value: string): string =>
  value.replace(/[\u0000-\u001f\u007f]/gu, "").slice(0, MOBILE_PAIRING_CODE_MAX)

const safeShareUrl = (value: string): string | null => {
  try {
    const parsed = new URL(value)
    return (parsed.protocol === "https:" || parsed.protocol === "http:") && parsed.username === "" && parsed.password === ""
      ? parsed.toString()
      : null
  } catch {
    return null
  }
}

export const decodeMobileShareUrl = (value: string): MobileShareIntake | null => {
  try {
    const parsed = new URL(value)
    if (parsed.protocol !== "openagents:" || parsed.hostname !== "share") return null
    const text = parsed.searchParams.get("text")?.trim() ?? ""
    const urlValue = parsed.searchParams.get("url")
    const url = urlValue === null ? null : safeShareUrl(urlValue)
    const titleValue = parsed.searchParams.get("title")?.trim() ?? ""
    const title = titleValue.length === 0 ? null : titleValue.slice(0, 200)
    if (text.length > MOBILE_SHARE_TEXT_MAX || (text.length === 0 && url === null) || (urlValue !== null && url === null)) return null
    return { title, text, url }
  } catch {
    return null
  }
}

export const mobileShareComposerText = (share: MobileShareIntake): string =>
  [share.title, share.text, share.url].filter((part): part is string => part !== null && part.length > 0).join("\n\n").slice(0, MOBILE_SHARE_TEXT_MAX)
