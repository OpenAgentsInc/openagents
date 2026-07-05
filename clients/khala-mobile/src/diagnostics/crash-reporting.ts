import type { ErrorInfo } from "react"

export type KhalaCrashReport = Readonly<{
  area: "render"
  componentStackPreview: string | null
  messageSafe: string
  name: string
}>

export type KhalaCrashReporter = (report: KhalaCrashReport) => void | Promise<void>

const MAX_DIAGNOSTIC_TEXT = 240

const SECRET_PATTERNS: ReadonlyArray<RegExp> = [
  /Bearer\s+[A-Za-z0-9._:-]+/g,
  /oa_agent_[A-Za-z0-9._:-]+/g,
  /sk-[A-Za-z0-9._:-]+/g,
  /\/Users\/[^\s)]+/g,
  /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g,
]

export const redactCrashDiagnosticText = (value: unknown): string => {
  const raw = value instanceof Error ? value.message : String(value)
  const redacted = SECRET_PATTERNS.reduce(
    (current, pattern) => current.replace(pattern, "[redacted]"),
    raw,
  ).replace(/\s+/g, " ").trim()
  return redacted.length <= MAX_DIAGNOSTIC_TEXT
    ? redacted
    : `${redacted.slice(0, MAX_DIAGNOSTIC_TEXT - 1)}…`
}

export const buildKhalaCrashReport = (
  error: unknown,
  errorInfo?: ErrorInfo,
): KhalaCrashReport => {
  const componentStack = errorInfo?.componentStack ?? ""
  return {
    area: "render",
    componentStackPreview:
      componentStack.trim().length === 0
        ? null
        : redactCrashDiagnosticText(componentStack),
    messageSafe: redactCrashDiagnosticText(error),
    name: error instanceof Error && error.name.trim().length > 0 ? error.name : "Error",
  }
}

export const noopKhalaCrashReporter: KhalaCrashReporter = () => undefined

export const reportKhalaMobileCrash = async (
  report: KhalaCrashReport,
  reporter: KhalaCrashReporter = noopKhalaCrashReporter,
): Promise<void> => {
  await reporter(report)
}
