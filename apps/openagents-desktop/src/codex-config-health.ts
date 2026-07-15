import { copyFileSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs"

import type { CodexChildSpawn } from "./codex-child-runtime.ts"

const OUTPUT_LIMIT = 16_000
const CHECK_TIMEOUT_MS = 5_000

export type CodexConfigurationIssue = Readonly<{
  path: string
  line: number
  column: number
  message: string
}>

export type CodexConfigurationHealth =
  | Readonly<{ state: "valid" }>
  | Readonly<{ state: "repaired"; issue: CodexConfigurationIssue; backupPath: string }>
  | Readonly<{ state: "invalid"; issue: CodexConfigurationIssue }>
  | Readonly<{ state: "unavailable"; detail: string }>

export const parseCodexConfigurationIssue = (text: string): CodexConfigurationIssue | null => {
  const match = text.match(/failed to load configuration:\s*(.+?):(\d+):(\d+):\s*([^\r\n]+)/iu)
  if (match === null) return null
  const line = Number(match[2])
  const column = Number(match[3])
  if (!Number.isSafeInteger(line) || line < 1 || !Number.isSafeInteger(column) || column < 1) return null
  return {
    path: match[1]!.trim().slice(0, 1_024),
    line,
    column,
    message: match[4]!.trim().slice(0, 1_000),
  }
}

const repairDisabledTransportlessMcpServer = (
  issue: CodexConfigurationIssue,
  now: () => Date,
): string | null => {
  if (!/invalid transport/iu.test(issue.message)) return null
  let source: string
  try { source = readFileSync(issue.path, "utf8") } catch { return null }
  const lines = source.split("\n")
  const target = Math.min(lines.length - 1, issue.line - 1)
  let start = target
  while (start >= 0 && !/^\s*\[[^\]]+\]\s*(?:#.*)?$/u.test(lines[start] ?? "")) start -= 1
  if (start < 0 || !/^\s*\[mcp_servers\.[A-Za-z0-9_-]+\]\s*(?:#.*)?$/u.test(lines[start] ?? "")) return null
  let end = start + 1
  while (end < lines.length && !/^\s*\[[^\]]+\]\s*(?:#.*)?$/u.test(lines[end] ?? "")) end += 1
  const body = lines.slice(start + 1, end)
    .map(line => line.replace(/\s+#.*$/u, "").trim())
    .filter(Boolean)
  if (body.length !== 1 || !/^enabled\s*=\s*false$/u.test(body[0]!)) return null

  const stamp = now().toISOString().replace(/[:.]/gu, "-")
  const backupPath = `${issue.path}.openagents-backup-${stamp}`
  const temporaryPath = `${issue.path}.openagents-repair-${process.pid}`
  try {
    const mode = statSync(issue.path).mode
    copyFileSync(issue.path, backupPath)
    const next = [...lines.slice(0, start), ...lines.slice(end)].join("\n")
    writeFileSync(temporaryPath, next, { encoding: "utf8", mode })
    renameSync(temporaryPath, issue.path)
    return backupPath
  } catch {
    return null
  }
}

const runCheck = (input: Readonly<{
  spawn: CodexChildSpawn
  env: Record<string, string | undefined>
  cwd: string
  timeoutMs: number
}>): Promise<Readonly<{ exitCode: number | null; output: string }>> => {
  const child = input.spawn({ args: ["mcp", "list"], env: input.env, cwd: input.cwd })
  if (child === null) return Promise.resolve({ exitCode: null, output: "Codex executable unavailable" })
  return new Promise(resolve => {
    let output = ""
    let settled = false
    const append = (chunk: Buffer | string): void => {
      if (output.length >= OUTPUT_LIMIT) return
      output += (typeof chunk === "string" ? chunk : chunk.toString("utf8")).slice(0, OUTPUT_LIMIT - output.length)
    }
    child.stdout?.on("data", append)
    child.stderr?.on("data", append)
    const finish = (exitCode: number | null): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve({ exitCode, output })
    }
    child.on("error", () => finish(null))
    child.on("close", (...args: unknown[]) => finish(typeof args[0] === "number" ? args[0] : null))
    const timer = setTimeout(() => {
      child.kill("SIGTERM")
      finish(null)
    }, input.timeoutMs)
  })
}

export const checkCodexConfiguration = async (input: Readonly<{
  spawn: CodexChildSpawn
  env: Record<string, string | undefined>
  cwd: string
  autoRepair?: boolean
  now?: () => Date
  timeoutMs?: number
}>): Promise<CodexConfigurationHealth> => {
  const checked = await runCheck({ ...input, timeoutMs: input.timeoutMs ?? CHECK_TIMEOUT_MS })
  if (checked.exitCode === 0) return { state: "valid" }
  const issue = parseCodexConfigurationIssue(checked.output)
  if (issue === null) return { state: "unavailable", detail: "Codex configuration check could not complete." }
  if (input.autoRepair !== false) {
    const backupPath = repairDisabledTransportlessMcpServer(issue, input.now ?? (() => new Date()))
    if (backupPath !== null) {
      const verified = await runCheck({ ...input, timeoutMs: input.timeoutMs ?? CHECK_TIMEOUT_MS })
      if (verified.exitCode === 0) return { state: "repaired", issue, backupPath }
    }
  }
  return { state: "invalid", issue }
}

export const formatCodexConfigurationIssue = (issue: CodexConfigurationIssue): string =>
  `${issue.path}:${issue.line}:${issue.column}: ${issue.message}`
