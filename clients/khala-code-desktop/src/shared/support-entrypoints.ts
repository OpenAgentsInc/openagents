import type {
  KhalaCodeDesktopDiagnosticsSnapshotResult,
  KhalaCodeDesktopUpdaterStatus,
} from "./rpc"

export type KhalaCodeSupportEntrypointId =
  | "release_notes"
  | "docs"
  | "support"
  | "feedback"
  | "bug_report"

export type KhalaCodeSupportEntrypoint = Readonly<{
  id: KhalaCodeSupportEntrypointId
  label: string
  url: string
}>

export type KhalaCodeSupportIssueMetadataInput = Readonly<{
  activeThreadPresent: boolean
  activeView: string
  diagnostics: KhalaCodeDesktopDiagnosticsSnapshotResult | null
  messageCount: number
  updater: KhalaCodeDesktopUpdaterStatus
}>

export type KhalaCodeSupportProjection = Readonly<{
  entries: readonly KhalaCodeSupportEntrypoint[]
  issueMetadata: string
}>

export const KHALA_CODE_SUPPORT_DOCS_URL = "https://openagents.com/docs"
export const KHALA_CODE_SUPPORT_URL = "https://github.com/OpenAgentsInc/openagents/discussions"
export const KHALA_CODE_FEEDBACK_URL = "https://github.com/OpenAgentsInc/openagents/issues/new"
export const KHALA_CODE_BUG_REPORT_URL =
  "https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml"
export const KHALA_CODE_RELEASES_URL = "https://github.com/OpenAgentsInc/openagents/releases"

const ALLOWED_SUPPORT_URLS: readonly RegExp[] = [
  /^https:\/\/openagents\.com(?:\/|$)/iu,
  /^https:\/\/github\.com\/OpenAgentsInc\/openagents(?:\/|$)/iu,
]

const SECRET_PATTERNS: readonly RegExp[] = [
  /\bsk-[A-Za-z0-9_-]{12,}\b/gu,
  /\bgh[pousr]_[A-Za-z0-9_]{20,}\b/gu,
  /\b(?:bearer|token|secret|api[_-]?key|authorization)\s*[:=]\s*[^\s,;]+/giu,
]

const PATH_PATTERNS: readonly RegExp[] = [
  /\/Users\/[^\s"'`]+/gu,
  /\/home\/[^\s"'`]+/gu,
  /[A-Za-z]:\\Users\\[^\s"'`]+/gu,
]

export const isKhalaCodeSupportUrlAllowed = (url: string): boolean => {
  try {
    const parsed = new URL(url)
    if (parsed.protocol !== "https:") return false
    return ALLOWED_SUPPORT_URLS.some(pattern => pattern.test(parsed.href))
  } catch {
    return false
  }
}

export const sanitizeKhalaCodeSupportMetadataText = (value: string): string => {
  let sanitized = value
  for (const pattern of SECRET_PATTERNS) sanitized = sanitized.replace(pattern, "[REDACTED_SECRET]")
  for (const pattern of PATH_PATTERNS) sanitized = sanitized.replace(pattern, "[REDACTED_LOCAL_PATH]")
  return sanitized
}

const releaseNotesUrlFor = (status: KhalaCodeDesktopUpdaterStatus): string =>
  isKhalaCodeSupportUrlAllowed(status.releaseNotesUrl)
    ? status.releaseNotesUrl
    : KHALA_CODE_RELEASES_URL

const updaterStateLabel = (status: KhalaCodeDesktopUpdaterStatus): string => {
  const state = status.state
  switch (state.status) {
    case "available":
    case "downloading":
    case "installing":
    case "ready":
      return `${state.status}:${state.version}`
    case "error":
      return `error:${sanitizeKhalaCodeSupportMetadataText(state.message)}`
    case "up_to_date":
      return `up_to_date:${state.version}`
    case "checking":
    case "idle":
      return state.status
  }
}

export const buildKhalaCodeSupportIssueMetadata = (
  input: KhalaCodeSupportIssueMetadataInput,
): string => {
  const diagnostics = input.diagnostics
  return sanitizeKhalaCodeSupportMetadataText([
    "Khala Code support metadata",
    `app=${input.updater.app}`,
    `version=${input.updater.currentVersion}`,
    `channel=${input.updater.channel}`,
    `updateState=${updaterStateLabel(input.updater)}`,
    `activeView=${input.activeView}`,
    `activeThreadPresent=${String(input.activeThreadPresent)}`,
    `visibleMessageCount=${String(input.messageCount)}`,
    `diagnosticsConfigured=${String(diagnostics !== null)}`,
    `diagnosticsUnresponsiveState=${diagnostics?.unresponsiveState ?? "unknown"}`,
    `diagnosticsRendererRows=${String(diagnostics?.counts.renderer ?? 0)}`,
    `diagnosticsMainRows=${String(diagnostics?.counts.main ?? 0)}`,
    "privateData=not_included",
  ].join("\n"))
}

export const projectKhalaCodeSupportEntrypoints = (
  input: KhalaCodeSupportIssueMetadataInput,
): KhalaCodeSupportProjection => {
  const entries: readonly KhalaCodeSupportEntrypoint[] = [
    {
      id: "release_notes",
      label: "Release Notes",
      url: releaseNotesUrlFor(input.updater),
    },
    {
      id: "docs",
      label: "Docs",
      url: KHALA_CODE_SUPPORT_DOCS_URL,
    },
    {
      id: "support",
      label: "Support",
      url: KHALA_CODE_SUPPORT_URL,
    },
    {
      id: "feedback",
      label: "Feedback",
      url: KHALA_CODE_FEEDBACK_URL,
    },
    {
      id: "bug_report",
      label: "Bug Report",
      url: KHALA_CODE_BUG_REPORT_URL,
    },
  ]
  return {
    entries: entries.filter(entry => isKhalaCodeSupportUrlAllowed(entry.url)),
    issueMetadata: buildKhalaCodeSupportIssueMetadata(input),
  }
}
